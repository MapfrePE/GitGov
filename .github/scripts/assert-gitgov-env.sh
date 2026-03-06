#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${GITGOV_ENV:-}" ]]; then
  echo "::error::GITGOV_ENV must be set explicitly in CI/deploy."
  exit 1
fi

case "${GITGOV_ENV}" in
  dev|development|local|test|testing|ci|staging|prod|production)
    ;;
  *)
    echo "::error::Unsupported GITGOV_ENV='${GITGOV_ENV}'."
    echo "::error::Allowed values: dev, development, local, test, testing, ci, staging, prod, production."
    exit 1
    ;;
esac

echo "GITGOV_ENV='${GITGOV_ENV}' validated for CI/deploy."
