#/bin/bash
pushd $(dirname $0) > /dev/null

ORIG=releases/burrowland_release.wasm
DEST=res/burrowland_release.wasm

echo "aaa" | openssl dgst -sha256 -binary > /dev/null 2>&1
C1=$?
echo "aaa" | base58 > /dev/null 2>&1
C2=$?
if [ ${C1} -eq 0 ] && [ ${C2} -eq 0 ]; then
    a=`cat ${ORIG} | openssl dgst -sha256 -binary | base58`
    if [ ! -f "${DEST}" ]; then
        echo "Compute hashcode for ${ORIG} ..."
        echo "${a}"
        popd > /dev/null
        exit 0
    fi

    echo "Comparing ${ORIG} with ${DEST} ..."
    b=`cat ${DEST} | openssl dgst -sha256 -binary | base58`
    if [ "${a}" = "${b}" ]; then
        echo "In releases: ${a}"
        echo "In res:      ${b}"
        echo 'codehash is identical.'
    else
        echo "In releases: ${a}"
        echo "In res:      ${b}"
        echo 'codehash is different.'
    fi

    popd > /dev/null
    exit 0
fi
echo "Not supported on this env"
