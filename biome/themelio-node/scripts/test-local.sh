#!/bin/bash

set -e

TESTDIR="$(dirname "${0}")"
PLAN_DIRECTORY="$(dirname "${TESTDIR}")"

bio pkg install --binlink themelio/bats
bio pkg install --binlink core/curl
bio pkg install --binlink core/nmap

cp "${PLAN_DIRECTORY}/plan-debug.sh" "${PLAN_DIRECTORY}/plan.sh"

pushd "${PLAN_DIRECTORY}"

source "plan.sh"

if [ -n "${SKIP_BUILD}" ]; then
  source "results/last_build.env"

  BIO_SVC_STATUS="$(bio svc status)"
  NO_SERVICES_LOADED="No services loaded."

  if [ "$BIO_SVC_STATUS" == "$NO_SERVICES_LOADED" ]; then
    bio pkg install --binlink --force "results/${pkg_artifact}"
    bio svc load "${pkg_ident}"
  else
    bio svc unload "${pkg_ident}" || true
    bio pkg install --binlink --force "results/${pkg_artifact}"
    sleep 1
    bio svc load "${pkg_ident}"
  fi
else
  build

  source "results/last_build.env"

  BIO_SVC_STATUS="$(bio svc status)"
  NO_SERVICES_LOADED="No services loaded."

  if [ "$BIO_SVC_STATUS" == "$NO_SERVICES_LOADED" ]; then
    bio pkg install --binlink --force "results/${pkg_artifact}"
    bio svc load "${pkg_ident}"
  else
    bio svc unload "${pkg_ident}" || true
    bio pkg install --binlink --force "results/${pkg_artifact}"
    sleep 1
    bio svc load "${pkg_ident}"
  fi
fi

echo "Sleeping for 5 seconds for the service to start."
sleep 5

if bats --print-output-on-failure "scripts/test-local.bats"; then
  rm "plan.sh"
  bio svc unload "${pkg_ident}"
else
  rm "plan.sh"
  bio svc unload "${pkg_ident}"
  exit 1
fi

popd

rm "${PLAN_DIRECTORY}/plan.sh"