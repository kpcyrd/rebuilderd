use crate::args::Args;
use clap::Parser;
use colored::Colorize;
use env_logger::Env;
use in_toto::crypto::{KeyType, PrivateKey, SignatureScheme};
use rebuilderd::attestation::{self, Attestation};
use rebuilderd::config::Config;
use rebuilderd_common::api::*;
use rebuilderd_common::config::*;
use rebuilderd_common::errors::*;
use rebuilderd_common::Status;
use rebuilderd_common::{PkgArtifact, PkgGroup, PkgRelease};
use serde_json::json;
use std::io;
use std::io::prelude::*;
use std::net::TcpStream;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

mod args;

async fn list_pkgs(client: &Client) -> Result<Vec<PkgRelease>> {
    client
        .list_pkgs(&ListPkgs {
            name: None,
            status: None,
            distro: None,
            suite: None,
            architecture: None,
        })
        .await
}

async fn initial_import(client: &Client) -> Result<()> {
    let distro = "archlinux".to_string();
    let suite = "core".to_string();
    let architecture = "x86_64".to_string();

    let url = "https://mirrors.kernel.org/archlinux/core/os/x86_64/zstd-1.4.5-1-x86_64.pkg.tar.zst"
        .to_string();
    let mut group = PkgGroup::new(
        "pkgbase".to_string(),
        "1.4.5-1".to_string(),
        distro.clone(),
        suite.clone(),
        architecture.clone(),
        None,
    );
    group.add_artifact(PkgArtifact {
        name: "zstd".to_string(),
        version: "1.4.5-1".to_string(),
        url,
    });
    let pkgs = vec![group];

    client
        .sync_suite(&SuiteImport {
            distro,
            suite,
            groups: pkgs,
        })
        .await?;

    Ok(())
}

async fn test<T: Sized>(label: &str, f: impl futures::Future<Output = Result<T>>) -> Result<T> {
    let mut stdout = io::stdout();
    write!(stdout, "{:70}", label)?;
    stdout.flush()?;

    let r = f.await;
    if r.is_ok() {
        println!("{}", "OK".green());
    } else {
        println!("{}", "ERR".red());
    }

    r
}

#[actix_web::main]
async fn spawn_server(config: Config, privkey: PrivateKey) {
    if let Err(err) = rebuilderd::run_config(config, privkey).await {
        error!("daemon errored: {:#}", err);
    }
}

fn wait_for_server(addr: &str) -> Result<()> {
    for _ in 0..100 {
        if TcpStream::connect(addr).is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    bail!("Failed to wait for daemon to start");
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let logging = match args.verbose {
        0 => "warn,rebuilderd_tests=info",
        1 => "info,rebuilderd_tests=debug",
        2 => "info,rebuilderd=debug,rebuilderd_tests=debug",
        3 => "debug",
        _ => "trace",
    };

    env_logger::init_from_env(Env::default().default_filter_or(logging));

    let addr = args.bind_addr;
    let endpoint = args.endpoint.unwrap_or_else(|| format!("http://{}", addr));

    let mut config = ConfigFile::default();

    config.auth.cookie = Some(args.cookie.clone());
    config.http.bind_addr = Some(addr.clone());
    config.endpoints.insert(
        endpoint.clone(),
        EndpointConfig {
            cookie: args.cookie.clone(),
        },
    );

    let privkey = PrivateKey::new(KeyType::Ed25519).expect("Failed to generate private key");
    let privkey = PrivateKey::from_pkcs8(&privkey, SignatureScheme::Ed25519)
        .expect("Failed to use generated private key");
    let pubkey = privkey.public().to_owned();

    if !args.no_daemon {
        let config = rebuilderd::config::from_struct(config.clone(), args.cookie)?;

        let tmp_dir = TempDir::new()?;
        info!("Changing cwd to {:?}", tmp_dir);
        std::env::set_current_dir(tmp_dir.path())?;

        info!("Spawning server");
        thread::spawn(|| {
            spawn_server(config, privkey);
        });
        wait_for_server(&addr)?;
    }

    info!("Setting up client for {:?}", endpoint);
    let mut client = Client::new(config.clone(), Some(endpoint))?;
    client.worker_key("worker1"); // TODO: this is not a proper key

    test("Testing database to be empty", async {
        let pkgs = list_pkgs(&client).await?;
        if !pkgs.is_empty() {
            bail!("Database is not empty");
        }
        Ok(())
    })
    .await?;

    test("Testing there is nothing to do", async {
        let task = client
            .pop_queue(&WorkQuery {
                supported_backends: vec!["archlinux".to_string()],
            })
            .await?;

        if task != JobAssignment::Nothing {
            bail!("Got a job assigned");
        }

        Ok(())
    })
    .await?;

    test("Sending initial import", async {
        initial_import(&client).await
    })
    .await?;

    test("Testing database to contain 1 pkg", async {
        let pkgs = list_pkgs(&client).await?;
        if pkgs.len() != 1 {
            bail!("Not 1");
        }
        Ok(())
    })
    .await?;

    test("Re-sending initial import", async {
        initial_import(&client).await
    })
    .await?;

    test("Testing database to still contain 1 pkg", async {
        let mut pkgs = list_pkgs(&client).await?;

        let pkg = pkgs.pop().ok_or_else(|| format_err!("No pkgs found"))?;

        if pkg.name != "zstd" {
            bail!("Mismatch name");
        }

        if pkg.status != Status::Unknown {
            bail!("Status not UNKWN");
        }

        if pkg.built_at.is_some() {
            bail!("Not None: built_at");
        }

        if !pkgs.is_empty() {
            bail!("Got more than 1 pkg bacK");
        }

        Ok(())
    })
    .await?;

    test("Fetching task and reporting BAD rebuild", async {
        let task = client
            .pop_queue(&WorkQuery {
                supported_backends: vec!["archlinux".to_string()],
            })
            .await?;

        let queue = match task {
            JobAssignment::Rebuild(item) => *item,
            _ => bail!("Expected a job assignment"),
        };

        let mut rebuilds = Vec::new();
        for artifact in queue.pkgbase.artifacts.clone() {
            rebuilds.push((
                artifact,
                Rebuild {
                    diffoscope: None,
                    status: BuildStatus::Bad,
                    attestation: None,
                },
            ));
        }

        let report = BuildReport {
            queue,
            build_log: String::new(),
            rebuilds,
        };
        client.report_build(&report).await?;

        Ok(())
    })
    .await?;

    test("Fetching pkg status", async {
        let mut pkgs = list_pkgs(&client).await?;

        let pkg = pkgs.pop().ok_or_else(|| format_err!("No pkgs found"))?;

        if pkg.status != Status::Bad {
            bail!("Unexpected pkg status");
        }

        if pkg.built_at.is_none() {
            bail!("Unexpected none: built_at");
        }

        Ok(())
    })
    .await?;

    test("Requeueing BAD pkgs", async {
        client
            .requeue_pkgs(&RequeueQuery {
                name: None,
                status: Some(Status::Bad),
                priority: 2,
                distro: None,
                suite: None,
                architecture: None,
                reset: false,
            })
            .await?;

        Ok(())
    })
    .await?;

    test("Fetching pkg status", async {
        let mut pkgs = list_pkgs(&client).await?;

        let pkg = pkgs.pop().ok_or_else(|| format_err!("No pkgs found"))?;

        if pkg.status != Status::Bad {
            bail!("Unexpected pkg status");
        }

        if pkg.built_at.is_none() {
            bail!("Unexpected none: built_at");
        }

        Ok(())
    })
    .await?;

    test("Fetching task and reporting GOOD rebuild", async {
        let task = client
            .pop_queue(&WorkQuery {
                supported_backends: vec!["archlinux".to_string()],
            })
            .await?;

        let queue = match task {
            JobAssignment::Rebuild(item) => *item,
            _ => bail!("Expected a job assignment"),
        };

        let mut rebuilds = Vec::new();
        for artifact in queue.pkgbase.artifacts.clone() {
            rebuilds.push((
                artifact,
                Rebuild {
                    diffoscope: None,
                    status: BuildStatus::Good,
                    attestation: None,
                },
            ));
        }

        let report = BuildReport {
            queue,
            build_log: String::new(),
            rebuilds,
        };
        client.report_build(&report).await?;

        Ok(())
    })
    .await?;

    test("Fetching pkg status", async {
        let mut pkgs = list_pkgs(&client).await?;

        let pkg = pkgs.pop().ok_or_else(|| format_err!("No pkgs found"))?;

        if pkg.status != Status::Good {
            bail!("Unexpected pkg status");
        }

        if pkg.built_at.is_none() {
            bail!("Unexpected none: built_at");
        }

        Ok(())
    })
    .await?;

    test("Sending import for build group of two artifacts", async {
        let distro = "rebuilderd".to_string();
        let suite = "main".to_string();
        let architecture = "x86_64".to_string();

        let mut group = PkgGroup::new(
            "hello-world".to_string(),
            "1.2.3-4".to_string(),
            distro.clone(),
            suite.clone(),
            architecture.clone(),
            Some("https://example.com/hello-world-1.2.3-4.buildinfo.txt".to_string()),
        );
        group.add_artifact(PkgArtifact {
            name: "foo".to_string(),
            version: "0.1.2".to_string(),
            url: "https://example.com/foo-0.1.2.tar.zst".to_string(),
        });
        group.add_artifact(PkgArtifact {
            name: "bar".to_string(),
            version: "0.3.4".to_string(),
            url: "https://example.com/bar-0.3.4.tar.zst".to_string(),
        });

        client
            .sync_suite(&SuiteImport {
                distro,
                suite,
                groups: vec![group],
            })
            .await?;

        Ok(())
    })
    .await?;

    test("Testing database to contain 3 pkgs", async {
        let pkgs = list_pkgs(&client).await?;
        if pkgs.len() != 3 {
            bail!("Not 3");
        }
        Ok(())
    })
    .await?;

    test("Fetching task and reporting GOOD with attestation", async {
        let task = client
            .pop_queue(&WorkQuery {
                supported_backends: vec!["rebuilderd".to_string()],
            })
            .await?;

        let queue = match task {
            JobAssignment::Rebuild(item) => *item,
            _ => bail!("Expected a job assignment"),
        };

        let mut rebuilds = Vec::new();
        for artifact in queue.pkgbase.artifacts.clone() {
            let attestation = serde_json::to_string(&json!({
                "signatures": [
                    {
                        "keyid": "c25d24c04760b6982de77736776edc6600d5f8e1e84d0bba2a7299959ce7d47f",
                        "sig": "8cd70318ea1b34c91bf7303e9c8811df43d1b4746aa9adf1d503ebb0241e0fbff9be28f36dac0318825782bf05dbbcea7171eb0ca9a89be3b02666f0f3c84301"
                    }
                ],
                "signed": {
                    "_type": "link",
                    "name": "rebuild spytrap-adb_0.3.5-1_amd64.deb",
                    "materials": {
                        "rust-spytrap-adb_0.3.5-1_amd64.buildinfo": {
                            "sha512": "d130dbdbd51480f5cb79c1e6ce09fa61a69766e56725543b9c19bee8248306b2c3c2a2c66b250992bf20b2f5af7cf03bf401255104714bc9d654126fb41bc59f",
                            "sha256": "9df2f9a721f5016874c5f78ae88d3df77f9e49ea6070f935bfeeb438cd73a158"
                        }
                    },
                    "products": {
                        "spytrap-adb_0.3.5-1_amd64.deb": {
                            "sha256": "58a7d451d5d59fda6284a05418b99e34fab32d07e63d0b164404eaaed1317edd",
                            "sha512": "f38806536701138cb1b2059565e5f73ec07288f9a3013ba986e33d510432e183e7bfe94af31bb8d480b85c84f4c145ed5c28c5949d618a4e94b2c7aecb309642"
                        }
                    },
                    "environment": null,
                    "byproducts": {},
                    "command": []
                },
            }))?;
            rebuilds.push((
                artifact,
                Rebuild {
                    diffoscope: None,
                    status: BuildStatus::Good,
                    attestation: Some(attestation),
                },
            ));
        }

        let report = BuildReport {
            queue,
            build_log: String::new(),
            rebuilds,
        };
        client.report_build(&report).await?;

        Ok(())
    })
    .await?;

    let public_keys = test("Fetching public keys", async {
        let response = client.fetch_public_keys().await?;

        let mut keys = Vec::new();
        for pem in response.current {
            for key in attestation::pem_to_pubkeys(pem.as_bytes())? {
                keys.push(key?);
            }
        }

        if keys != [pubkey.clone()] {
            bail!("Wrong api response, expected key={pubkey:?}, received={keys:?}");
        }

        Ok(keys)
    })
    .await?;

    test("Fetching and verify attestation", async {
        let pkgs = list_pkgs(&client).await?;

        for pkg in pkgs {
            if !pkg.has_attestation {
                continue;
            }

            let Some(build_id) = pkg.build_id else {
                continue;
            };

            let attestation = client.fetch_attestation(build_id).await?;
            let attestation = Attestation::parse(&attestation)?;

            // ensure the attestation verifies because the rebuilderd instance itself signed it too
            attestation.verify(1, &public_keys)?;
        }

        Ok(())
    })
    .await?;

    Ok(())
}
