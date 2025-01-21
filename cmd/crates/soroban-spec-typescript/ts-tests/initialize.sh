#!/bin/bash

# read .env file, but prefer explicitly set environment variables
IFS=$'\n'
for l in $(cat .env); do
    IFS='=' read -ra VARVAL <<< "$l"
    # If variable with such name already exists, preserves its value
    eval "export ${VARVAL[0]}=\${${VARVAL[0]}:-${VARVAL[1]}}"
done
unset IFS

echo Network
echo "  RPC:        $STELLAR_RPC_URL"
echo "  Passphrase: \"$STELLAR_NETWORK_PASSPHRASE\""

NETWORK_STATUS=$(curl -s -X POST "http://localhost:8000/rpc" -H "Content-Type: application/json" -d '{ "jsonrpc": "2.0", "id": 8675309, "method": "getHealth" }' | sed 's/.*"status":"\([^"]*\)".*/\1/') || { echo "Make sure you're running local RPC network on localhost:8000" && exit 1; }
echo "  Status:     $NETWORK_STATUS"

if [[ "$NETWORK_STATUS" != "healthy" ]]; then
  echo "Network is not healthy (not running?), exiting"
  exit 1
fi

# Print command before executing, from https://stackoverflow.com/a/23342259/249801
# Discussion: https://github.com/stellar/stellar-tools/pull/1034#pullrequestreview-1690667116
exe() { echo"${@/eval/}" ; "$@" ; }

function fund_all() {
  exe eval "./stellar keys generate --fund root"
}
function upload() {
  exe eval "(./stellar contract $1 --quiet --source root --wasm $2 --ignore-checks) > $3"
}
function deploy_all() {
  upload deploy ../../../../target/wasm32-unknown-unknown/test-wasms/test_custom_types.wasm contract-id-custom-types.txt
  upload install ../../../../target/wasm32-unknown-unknown/test-wasms/test_constructor.wasm contract-wasm-hash-constructor.txt
  exe eval "./stellar contract asset deploy --asset native --source root"
}
function bind() {
  exe eval "./stellar contract bindings typescript $1 $2 --output-dir ./node_modules/$3 --overwrite"
  exe eval "sh -c \"cd ./node_modules/$3 && npm install && npm run build\""
}
function bind_all() {
  bind --contract-id $(cat contract-id-custom-types.txt) test-custom-types
  bind --wasm-hash $(cat contract-wasm-hash-constructor.txt) test-constructor
  bind --contract-id $(./stellar contract id asset --asset native) xlm
}

fund_all
deploy_all
bind_all
