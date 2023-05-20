type callbacks = {
  onError: Function[]
  onSuccess: Function[]
  onComplete: Function[]
}

export async function execAndCallback(
  operation: () => Promise<any>,
  _funcs: callbacks | (() => callbacks),
) {
  try {
    const result = await operation()
    const funcs = typeof _funcs === 'function' ? _funcs() : _funcs
    for (const cb of [...funcs.onSuccess, ...funcs.onComplete]) {
      await Promise.resolve(cb()).catch(err => {
        console.error(err)
        console.warn('Uncaught error in DB transaction success callback')
      })
    }
    return result
  } catch (err) {
    const funcs = typeof _funcs === 'function' ? _funcs() : _funcs
    for (const cb of [...funcs.onError, ...funcs.onComplete]) {
      await Promise.resolve(cb()).catch(err => {
        console.error(err)
        console.warn('Uncaught error in DB transaction error callback')
      })
    }
    throw err
  }
}
