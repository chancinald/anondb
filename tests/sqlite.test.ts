/* eslint-disable jest/no-hooks, jest/valid-describe */
import testSchema from './test-schema'
import { DB, SQLiteConnector, TableData } from '../src/node'
import FindTests from './database/find'
import CreateTests from './database/create'
import UpdateTests from './database/update'
import DeleteTests from './database/delete'
import TransactionTests from './database/transaction'
import assert from 'assert'
import fs from 'fs'
import path from 'path'
import os from 'os'

describe('sqlite tests', function(this: { db: DB }) {
  beforeEach(async () => {
    this.db = await SQLiteConnector.create(testSchema, ':memory:')
    for (const { name } of testSchema) {
      await this.db.delete(name, {
        where: {},
      })
    }
  })

  afterEach(async () => {
    await this.db.close()
  })

  FindTests.bind(this)()
  CreateTests.bind(this)()
  UpdateTests.bind(this)()
  DeleteTests.bind(this)()
  TransactionTests.bind(this)()

  // Test migrating a schema

  test('should migrate a table', async () => {
    const dbPath = path.join(await fs.promises.mkdtemp(path.join(os.tmpdir(), 'anondb-')), 'db.sqlite')
    {
      const schema = [
        {
          name: 'TestTable',
          primaryKey: 'id',
          rows: [
            {
              name: 'id',
              type: 'String',
              default: () => Math.floor(Math.random() * 1000000).toString(),
            },
            ['uniqueField', 'String', { unique: true }],
            ['uniqueAndOptionalField', 'String', { unique: true, optional: true }],
            ['optionalField', 'String', { optional: true }],
            ['regularField', 'String'],
            ['f1', 'Bool'],
            ['f2', 'Int'],
          ],
        },
      ] as TableData[]
      const db = await SQLiteConnector.create(schema, dbPath)
      await db.create('TestTable', {
        uniqueField: '0',
        regularField: 'hello',
        f1: true,
        f2: 2190
      })
      await db.close()
    }
    {
      const schema = [
        {
          name: 'TestTable',
          primaryKey: 'id',
          rows: [
            {
              name: 'id',
              type: 'String',
              default: () => Math.floor(Math.random() * 1000000).toString(),
            },
            ['uniqueField', 'String', { unique: true }],
            ['uniqueAndOptionalField', 'String', { unique: true, optional: true }],
            ['optionalField', 'String', { optional: true }],
            ['f1', 'Bool'],
            ['f2', 'Int'],
            {
              name: 'newField',
              type: 'String',
              default: () => 'newval'
            }
          ],
        },
      ] as TableData[]
      const db = await SQLiteConnector.create(schema, dbPath)
      const docs = await db.findMany('TestTable', {
        where: {}
      })
      assert.equal(docs.length, 1)
      const [doc] = docs
      assert.equal(doc.newField, 'newval')
      assert.equal(doc.regularField, undefined)
      await db.close()
    }
  })

  test('should fail to migrate a schema', async () => {
    const dbPath = path.join(await fs.promises.mkdtemp(path.join(os.tmpdir(), 'anondb-')), 'db.sqlite')
    {
      const schema = [
        {
          name: 'TestTable',
          primaryKey: 'id',
          rows: [
            {
              name: 'id',
              type: 'String',
              default: () => Math.floor(Math.random() * 1000000).toString(),
            },
            ['uniqueField', 'String', { unique: true }],
            ['uniqueAndOptionalField', 'String', { unique: true, optional: true }],
            ['optionalField', 'String', { optional: true }],
            ['regularField', 'String'],
          ],
        },
      ] as TableData[]
      const db = await SQLiteConnector.create(schema, dbPath)
      await db.create('TestTable', {
        uniqueField: '0',
        regularField: 'hello'
      })
      await db.close()
    }
    {
      const schema = [
        {
          name: 'TestTable',
          primaryKey: 'id',
          rows: [
            {
              name: 'id',
              type: 'String',
              default: () => Math.floor(Math.random() * 1000000).toString(),
            },
            ['uniqueField', 'String', { unique: true }],
            ['uniqueAndOptionalField', 'String', { unique: true, optional: true }],
            ['optionalField', 'String', { optional: true }],
            {
              name: 'newField',
              type: 'String',
            }
          ],
        },
      ] as TableData[]
      try {
        const db = await SQLiteConnector.create(schema, dbPath)
      } catch (err) {
        assert.notEqual(err.toString().indexOf('newField must be either optional'), -1)
      }
    }
  })
})
