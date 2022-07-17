/* eslint-disable jest/require-top-level-describe, no-plusplus, jest/no-export */
import assert from 'assert'
import { DB } from '../../src'

export default function(this: { db: DB }) {
  test('should delete one', async () => {
    const table = 'Table7'
    await this.db.create(table, {
      id: 0,
      boolField: true,
      stringField: 'test',
      objectField: {},
    })
    await this.db.create(table, {
      id: 1,
      boolField: true,
      stringField: 'test',
      objectField: {},
    })
    const deleted = await this.db.delete(table, {
      where: {
        id: 0,
        boolField: true,
      },
    })
    assert.equal(deleted, 1)
    const count = await this.db.count(table, {})
    assert.equal(count, 1)
  })

  test('should delete (undefined/null)', async () => {
    const table = 'Table7'
    await this.db.create(table, {
      id: 0,
      boolField: true,
      stringField: 'test',
      objectField: {},
    })
    await this.db.create(table, {
      id: 1,
      boolField: true,
      stringField: 'test',
      objectField: {},
    })
    {
      const deleted = await this.db.delete(table, {
        where: {
          id: null,
        },
      })
      assert.equal(deleted, 0)
      const count = await this.db.count(table, {})
      assert.equal(count, 2)
    }
    {
      const deleted = await this.db.delete(table, {
        where: {
          id: undefined,
        },
      })
      assert.equal(deleted, 2)
      const count = await this.db.count(table, {})
      assert.equal(count, 0)
    }
  })

  test('should delete many', async () => {
    const table = 'Table7'
    await this.db.create(table, {
      id: 0,
      boolField: true,
      stringField: 'test',
      objectField: {},
    })
    await this.db.create(table, {
      id: 1,
      boolField: true,
      stringField: 'test',
      objectField: {},
    })
    {
      const deleted = await this.db.delete(table, {
        where: {
          boolField: false,
        },
      })
      assert.equal(deleted, 0)
      const count = await this.db.count(table, {})
      assert.equal(count, 2)
    }
    {
      const deleted = await this.db.delete(table, {
        where: {
          boolField: true,
        },
      })
      assert.equal(deleted, 2)
      const count = await this.db.count(table, {})
      assert.equal(count, 0)
    }
    await this.db.create(table, {
      id: 0,
      boolField: true,
      stringField: 'test',
      objectField: {},
    })
    await this.db.create(table, {
      id: 1,
      boolField: true,
      stringField: 'test',
      objectField: {},
    })
    {
      const deleted = await this.db.delete(table, {
        where: {},
      })
      assert.equal(deleted, 2)
      const count = await this.db.count(table, {})
      assert.equal(count, 0)
    }
  })

  test('should catch delete errors', async () => {
    const table = 'Table7'
    try {
      await this.db.delete(table, {
        where: {
          invalidField: 0,
        },
      })
      assert(false)
    } catch (err) {
      assert(/Error: anondb error: Error: Unable to find row definition for key: "invalidField"/.test(err.toString()))
    }

  })
}
