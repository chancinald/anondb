mod clear;
mod empty;
mod insert;
mod range;
mod remove;
mod transaction;

use anyhow::Result;
use domacro::domacro;

use crate::*;

#[macro_export]
macro_rules! all_kv_impls {
    ($fn_name:ident) => {
        paste::paste! {
            #[test]
            fn [<redb_ $fn_name>]() -> Result<()> {
                let kv = RedbKV::in_memory(None)?;
                $fn_name(&kv)?;
                Ok(())
            }
        }
    };
}

#[macro_export]
macro_rules! all_read_impls {
    ($fn_name:ident) => {
        paste::paste! {
            #[test]
            fn [<redb_ $fn_name>]() -> Result<()> {
                let kv = RedbKV::in_memory(None)?;
                $fn_name(&kv)?;
                Ok(())
            }

            #[test]
            fn [<redb_read_tx_ $fn_name>]() -> Result<()> {
                let kv = RedbKV::in_memory(None)?;
                let read = kv.read_tx()?;
                $fn_name(&read)?;
                Ok(())
            }

            #[test]
            fn [<redb_write_tx_ $fn_name>]() -> Result<()> {
                let kv = RedbKV::in_memory(None)?;
                let write = kv.write_tx()?;
                $fn_name(&write)?;
                write.commit()?;
                Ok(())
            }
        }
    };
}

#[macro_export]
macro_rules! all_read_write_impls {
    ($fn_name:ident) => {
        paste::paste! {
            #[test]
            fn [<redb_ $fn_name>]() -> Result<()> {
                let kv = RedbKV::in_memory(None)?;
                $fn_name(&kv)?;
                Ok(())
            }

            #[test]
            fn [<redb_write_tx_ $fn_name>]() -> Result<()> {
                let kv = RedbKV::in_memory(None)?;
                let write = kv.write_tx()?;
                $fn_name(&write)?;
                write.commit()?;
                Ok(())
            }
        }
    };
}
