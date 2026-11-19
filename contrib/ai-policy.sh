#!/usr/bin/env bash
set -euo pipefail

{
    git log --format='Commit: %h%nAuthor: %an <%ae>%nCommitter: %cn <%ce>%n%B' \
        | grep -Ei '^(Author:|Committer:|Co-authored-by:)'

} | if grep -Ei 'Claude.*(anthropic.com|claude.ai)|Codex.*openai.com|Copilot.*github.com'; then

    echo "::error::AI Model Credited as Author"
    echo "::error::AI-assisted contributions and disclosure of AI usage are allowed, but AI models may not be credited as authors or co-authors. Authorship is reserved for humans."
    exit 1
fi

echo "Authorship attribution check passed."
