macro_rules! import_models {
    ($x:ident) => {
        mod $x;
        pub use self::$x::*;
    };
}

import_models!(rebuild);
import_models!(rebuild_artifact);
import_models!(binary_package);
import_models!(build_input);
import_models!(source_package);
import_models!(worker);
import_models!(queue);
