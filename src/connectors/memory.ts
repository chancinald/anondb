/* eslint-disable @typescript-eslint/no-unused-vars */
import AsyncLock from 'async-lock'
import _structuredClone from '@ungap/structured-clone'
import {
  DB,
  Schema,
  FindManyOptions,
  FindOneOptions,
  WhereClause,
  UpdateOptions,
  UpsertOptions,
  DeleteManyOptions,
  TransactionDB
} from '../types'
import { validateDocuments, matchDocument } from '../helpers/memory'
import { loadIncluded } from '../helpers/shared'
import { execAndCallback } from '../helpers/callbacks'

export class MemoryConnector extends DB {
  schema: Schema = {}
  lock = new AsyncLock({ maxPending: 100000 })

  db = {
    __uniques__: {}
  }

  constructor(schema: Schema) {
    super()
    this.schema = schema
    for (const key of Object.keys(schema)) {
      this.db[key] = []
      for (const row of this.uniqueRows(key)) {
        this.db.__uniques__[this.uniqueRowKey(key, row.name)] = {}
      }
    }
  }

  uniqueRowKey(collection: string, row: string) {
    return `unique-${collection}-${row}`
  }

  // check if a row is unique or a primary key
  uniqueRows(_collection: string) {
    const collection = this.schema[_collection]
    if (!collection) {
      throw new Error(`Invalid collection: "${_collection}"`)
    }
    const rows = [] as any[]
    for (const row of collection.rows) {
      if (row.unique || [collection.primaryKey].flat().indexOf(row.name) !== -1)
        rows.push(row)
    }
    return rows
  }

  checkForInvalidRows(_collection: string, doc: any, checkingWhere = false) {
    const collection = this.schema[_collection]
    if (!collection) {
      throw new Error(`Invalid collection: "${_collection}"`)
    }
    // check for invalid fields
    for (const d of [doc].flat()) {
      for (const key of Object.keys(d)) {
        if (!collection.rowsByName[key] && key !== 'OR' && key !== 'AND') {
          throw new Error(`Unable to find row definition for key: "${key}"`)
        } else if (!collection.rowsByName[key] && !checkingWhere) {
          throw new Error(`Unable to find row definition for key: "${key}"`)
        }
      }
    }
  }

  async create(collection: string, doc: any) {
    return this.lock.acquire('write', async () =>
      this._create(collection, doc)
    )
  }

  async _create(_collection: string, doc: any) {
    const collection = this.schema[_collection]
    if (!collection) {
      throw new Error(`Invalid collection: "${_collection}"`)
    }
    // check for invalid fields
    this.checkForInvalidRows(_collection, doc)
    const docs = validateDocuments(collection, doc)
    const newUniques = {}
    // now we've finalized the documents, compare uniqueness within the set
    for (const row of this.uniqueRows(_collection)) {
      newUniques[this.uniqueRowKey(_collection, row.name)] = {}
    }
    // make a copy to operate on
    for (const d of docs) {
      for (const row of this.uniqueRows(_collection)) {
        if (
          newUniques[this.uniqueRowKey(_collection, row.name)][d[row.name]] ||
          this.db.__uniques__[this.uniqueRowKey(_collection, row.name)][d[row.name]]
        ) {
          throw new Error(`Uniqueness constraint violation for row "${row.name}"`)
        }
        newUniques[this.uniqueRowKey(_collection, row.name)][d[row.name]] = true
      }
    }
    // all checks pass, start mutating
    for (const d of docs) {
      this.db[_collection].push(d)
    }
    for (const key of Object.keys(newUniques)) {
      this.db.__uniques__[key] = { ...this.db.__uniques__[key], ...newUniques[key]}
    }
    if (docs.length === 1) {
      return docs[0]
    } else {
      return docs
    }
  }

  async findMany(_collection: string, options: FindManyOptions) {
    const collection = this.schema[_collection]
    if (!collection) {
      throw new Error(`Invalid collection: "${_collection}"`)
    }
    this.checkForInvalidRows(_collection, options.where, true)
    const matches = [] as any[]
    for (const doc of this.db[_collection]) {
      if (matchDocument(options.where, doc)) {
        // make sure not to mutate stuff outside of this
        matches.push({ ...doc })
      }
    }
    const sortKeys = Object.keys(options.orderBy || {})
    if (sortKeys.length > 0) {
      // do some ordering
      const sortKey = sortKeys[0]
      matches.sort((a, b) => {
        if (a[sortKey] > b[sortKey]) {
          return (options.orderBy || {})[sortKey] === 'asc' ? 1 : -1
        } else if (a[sortKey] < b[sortKey]) {
          return (options.orderBy || {})[sortKey] === 'asc' ? -1 : 1
        }
        return 0
      })
    }
    await loadIncluded(_collection, {
      models: matches,
      include: options.include,
      findMany: this.findMany.bind(this),
      table: collection
    })
    return matches
  }

  async findOne(collection: string, options: FindOneOptions) {
    const docs = await this.findMany(collection, options)
    if (docs.length > 0) {
      return docs[0]
    }
    return null
  }

  async count(collection: string, where: WhereClause) {
    const docs = await this.findMany(collection, { where })
    return docs.length
  }

  async update(collection: string, options: UpdateOptions) {
    return this.lock.acquire('write', async () =>
      this._update(collection, options)
    )
      .catch(err => {
        throw new Error(`anondb error: ${err}`)
      })
  }

  async _update(_collection: string, options: UpdateOptions) {
    const collection = this.schema[_collection]
    if (!collection) {
      throw new Error(`Invalid collection: "${_collection}"`)
    }
    let updatedCount = 0
    const newDocs = [] as any[]
    this.checkForInvalidRows(_collection, options.update)

    // deep copy for the operation
    const newUniques = {}
    for (const row of this.uniqueRows(_collection)) {
      newUniques[this.uniqueRowKey(_collection, row.name)] = {
        ...this.db.__uniques__[this.uniqueRowKey(_collection, row.name)]
      }
    }

    for (const doc of this.db[_collection]) {
      if (matchDocument(options.where, doc)) {
        updatedCount++
        const newDoc = {
          ...doc,
          ...options.update,
        }
        // first undo the uniques in the old doc
        for (const row of this.uniqueRows(_collection)) {
          delete newUniques[this.uniqueRowKey(_collection, row.name)][doc[row.name]]
        }
        // then add the new uniques from the new document
        // check when adding the new uniques
        for (const row of this.uniqueRows(_collection)) {
          if (newUniques[this.uniqueRowKey(_collection, row.name)][doc[row.name]]) {
            // we have a double unique
            throw new Error('Unique constraint violation')
          }
          newUniques[this.uniqueRowKey(_collection, row.name)][doc[row.name]] = true
        }
        newDocs.push(newDoc)
      } else {
        newDocs.push(doc)
      }
    }
    this.db[_collection] = newDocs
    for (const key of Object.keys(newUniques)) {
      this.db.__uniques__[key] = newUniques[key]
    }
    return updatedCount
  }

  async upsert(collection: string, options: UpsertOptions) {
    return this.lock.acquire('write', () => this._upsert(collection, options))
      .catch(err => {
        throw new Error(`anondb error: ${err}`)
      })
  }

  async _upsert(collection: string, options: UpsertOptions) {
    const updatedCount = await this._update(collection, options)
    if (updatedCount > 0) {
      return Object.keys(options.update).length === 0 ? 0 : updatedCount
    }
    const created = await this._create(collection, options.create)
    return Array.isArray(created) ? created.length : 1
  }

  async delete(collection: string, options: DeleteManyOptions) {
    return this.lock.acquire('write', () =>
      this._delete(collection, options)
    )
      .catch(err => {
        throw new Error(`anondb error: ${err}`)
      })
  }

  async _delete(_collection: string, options: DeleteManyOptions) {
    const collection = this.schema[_collection]
    if (!collection) {
      throw new Error(`Invalid collection: "${_collection}"`)
    }
    this.checkForInvalidRows(_collection, options.where)
    const newUniques = {}
    for (const row of this.uniqueRows(_collection)) {
      newUniques[this.uniqueRowKey(_collection, row.name)] = {
        ...this.db.__uniques__[this.uniqueRowKey(_collection, row.name)]
      }
    }
    const newDocs = [] as any[]
    for (const doc of this.db[_collection]) {
      if (!matchDocument(options.where, doc)) {
        newDocs.push(doc)
      } else {
        for (const row of this.uniqueRows(_collection)) {
          delete newUniques[this.uniqueRowKey(_collection, row.name)][doc[row.name]]
        }
      }
    }
    const deletedCount = this.db[_collection].length - newDocs.length
    this.db[_collection] = newDocs
    for (const key of Object.keys(newUniques)) {
      this.db.__uniques__[key] = newUniques[key]
    }
    return deletedCount
  }

  async transaction(operation: (db: TransactionDB) => void, onComplete?: () => void) {
    return this.lock.acquire('write', () =>
      this._transaction(operation, onComplete)
    )
      .catch(err => {
        throw new Error(`anondb error: ${err}`)
      })
  }

  async _transaction(operation: (db: TransactionDB) => void, onComplete?: () => void) {
    const onCommitCallbacks = [] as any[]
    const onErrorCallbacks = [] as any[]
    const onCompleteCallbacks = [] as any[]
    if (onComplete) onCompleteCallbacks.push(onComplete)

    const txThis = {
      schema: this.schema,
      db: _structuredClone(this.db),
    } as any
    txThis._delete = this._delete.bind(txThis)
    txThis._create = this._create.bind(txThis)
    txThis._update = this._update.bind(txThis)
    txThis._upsert = this._upsert.bind(txThis)
    txThis.findOne = this.findOne.bind(txThis)
    txThis.findMany = this.findMany.bind(txThis)
    txThis.uniqueRows = this.uniqueRows.bind(txThis)
    txThis.uniqueRowKey = this.uniqueRowKey.bind(txThis)
    txThis.checkForInvalidRows = this.checkForInvalidRows.bind(txThis)
    const tx = async () => {
      let promise = Promise.resolve()
      // deep copy the database for doing operations on
      const db = {
        delete: (collection: string, options: DeleteManyOptions) => {
          promise = promise.then(() => txThis._delete(collection, options))
        },
        create: (collection: string, docs: any) => {
          promise = promise.then(() => txThis._create(collection, docs))
        },
        update: (collection: string, options: UpdateOptions) => {
          promise = promise.then(() => txThis._update(collection, options))
        },
        upsert: (collection: string, options: UpsertOptions) => {
          promise = promise.then(() => txThis._upsert(collection, options))
        },
        onCommit: (cb: Function) => {
          if (typeof cb !== 'function')
            throw new Error('Non-function onCommit callback supplied')
          onCommitCallbacks.push(cb)
        },
        onError: (cb: Function) => {
          if (typeof cb !== 'function')
            throw new Error('Non-function onError callback supplied')
          onErrorCallbacks.push(cb)
        },
        onComplete: (cb: Function) => {
          if (typeof cb !== 'function')
            throw new Error('Non-function onComplete callback supplied')
          onCompleteCallbacks.push(cb)
        },
      } as TransactionDB
      await Promise.all([
        Promise.resolve(operation(db)),
        promise,
      ])
      this.db = txThis.db
    }
    await execAndCallback(tx,
      () => ({
        onError: onErrorCallbacks,
        onSuccess: onCommitCallbacks,
        onComplete: [...onCompleteCallbacks, () => {
          for (const key of Object.keys(txThis)) {
            delete txThis[key]
          }
        }]
      })
    )
  }

  async close() {
    // no-op, it's just a variable
  }

  async closeAndWipe() {
    for (const key of Object.keys(this.db)) {
      this.db[key] = []
    }
  }
}
