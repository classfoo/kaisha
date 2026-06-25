# Shared environment defaults for local development.
# Override any value in the shell before running dev scripts.
export KAISHA_LOG="${KAISHA_LOG:-info,server=debug,hyper=warn,tower=warn,axum=warn}"
