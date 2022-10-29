/* eslint-disable jest/no-hooks, jest/valid-describe */
// import assert from 'assert'
import testSchema from './test-schema'
import { DB, MemoryConnector } from '../src/web'
import { constructSchema } from '../src/types'
import FindTests from './database/find'
import CreateTests from './database/create'
import UpdateTests from './database/update'
import DeleteTests from './database/delete'
import TransactionTests from './database/transaction'

describe('memory tests', function(this: any) {
  this.db = {} as DB
  beforeEach(async () => {
    this.db = new MemoryConnector(constructSchema(testSchema))
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
})
