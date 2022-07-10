const { Web3Storage, getFilesFromPath } = require('web3.storage')
const path = require('path')
const fetch = require('node-fetch')

;(async () => {
  const client = new Web3Storage({
    token: process.env.WEB3_STORAGE_TOKEN,
  })

  const files = await getFilesFromPath(path.join(__dirname, '../coverage/lcov-report/'))
  const rootCid = await client.put(files, {
    wrapWithDirectory: false,
  })
  await fetch('https://storage.jchancehud.workers.dev/update', {
    method: 'POST',
    headers: {
      'content-type': 'application/json'
    },
    body: JSON.stringify({
      key: process.env.STORAGE_KEY,
      name: 'anondb',
      target: `https://${rootCid}.ipfs.dweb.link/`
    })
  })
})()
