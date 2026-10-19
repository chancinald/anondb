mod clear;
mod empty;
mod insert;
mod list_tables;
mod range;
mod remove;
mod transaction;

use anyhow::Result;
use domacro::domacro;

use super::*;

pub fn rand_utf8(len: usize) -> String {
    vec![char::default(); len]
        .into_iter()
        .map(|_| rand::random::<char>())
        .collect()
}

#[macro_export]
macro_rules! all_kv_impls {
    ($fn_name:ident) => {
        paste::paste! {
            #[tokio::test]
            async fn [<memory_ $fn_name>]() -> Result<()> {
                let kv = MemoryKV::new(None).await?;
                $fn_name(&kv).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<any_memory_ $fn_name>]() -> Result<()> {
                let kv = AnyKV::new(None).await?;
                $fn_name(&kv).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<redb_ $fn_name>]() -> Result<()> {
                let kv = RedbKV::new(None).await?;
                $fn_name(&kv).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<any_redb_ $fn_name>]() -> Result<()> {
                let kv = AnyKV::from(RedbKV::new(None).await?);
                $fn_name(&kv).await?;
                Ok(())
            }
        }
    };
}

#[macro_export]
macro_rules! all_read_impls {
    ($fn_name:ident) => {
        paste::paste! {
            #[tokio::test]
            async fn [<memory_ $fn_name>]() -> Result<()> {
                let kv = MemoryKV::new(None).await?;
                $fn_name(&kv).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<any_memory_ $fn_name>]() -> Result<()> {
                let kv = AnyKV::new(None).await?;
                $fn_name(&kv).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<memory_read_tx_ $fn_name>]() -> Result<()> {
                let kv = MemoryKV::new(None).await?;
                let read = kv.read_tx().await?;
                $fn_name(&read).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<any_memory_read_tx_ $fn_name>]() -> Result<()> {
                let kv = AnyKV::new(None).await?;
                let read = kv.read_tx().await?;
                $fn_name(&read).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<memory_write_tx_ $fn_name>]() -> Result<()> {
                let kv = MemoryKV::new(None).await?;
                let write = kv.write_tx().await?;
                $fn_name(&write).await?;
                write.commit().await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<any_memory_write_tx_ $fn_name>]() -> Result<()> {
                let kv = AnyKV::new(None).await?;
                let write = kv.write_tx().await?;
                $fn_name(&write).await?;
                write.commit().await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<redb_ $fn_name>]() -> Result<()> {
                let kv = RedbKV::new(None).await?;
                $fn_name(&kv).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<any_redb_ $fn_name>]() -> Result<()> {
                let kv = AnyKV::from(RedbKV::new(None).await?);
                $fn_name(&kv).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<redb_read_tx_ $fn_name>]() -> Result<()> {
                let kv = RedbKV::new(None).await?;
                let read = kv.read_tx().await?;
                $fn_name(&read).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<any_redb_read_tx_ $fn_name>]() -> Result<()> {
                let kv = AnyKV::from(RedbKV::new(None).await?);
                let read = kv.read_tx().await?;
                $fn_name(&read).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<redb_write_tx_ $fn_name>]() -> Result<()> {
                let kv = RedbKV::new(None).await?;
                let write = kv.write_tx().await?;
                $fn_name(&write).await?;
                write.commit().await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<any_redb_write_tx_ $fn_name>]() -> Result<()> {
                let kv = AnyKV::from(RedbKV::new(None).await?);
                let write = kv.write_tx().await?;
                $fn_name(&write).await?;
                write.commit().await?;
                Ok(())
            }
        }
    };
}

#[macro_export]
macro_rules! all_read_write_impls {
    ($fn_name:ident) => {
        paste::paste! {
            #[tokio::test]
            async fn [<memory_ $fn_name>]() -> Result<()> {
                let kv = MemoryKV::new(None).await?;
                $fn_name(&kv).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<any_memory_ $fn_name>]() -> Result<()> {
                let kv = AnyKV::new(None).await?;
                $fn_name(&kv).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<memory_write_tx_ $fn_name>]() -> Result<()> {
                let kv = MemoryKV::new(None).await?;
                let write = kv.write_tx().await?;
                $fn_name(&write).await?;
                write.commit().await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<any_memory_write_tx_ $fn_name>]() -> Result<()> {
                let kv = AnyKV::new(None).await?;
                let write = kv.write_tx().await?;
                $fn_name(&write).await?;
                write.commit().await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<redb_ $fn_name>]() -> Result<()> {
                let kv = RedbKV::new(None).await?;
                $fn_name(&kv).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<any_redb_ $fn_name>]() -> Result<()> {
                let kv = AnyKV::from(RedbKV::new(None).await?);
                $fn_name(&kv).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<redb_write_tx_ $fn_name>]() -> Result<()> {
                let kv = RedbKV::new(None).await?;
                let write = kv.write_tx().await?;
                $fn_name(&write).await?;
                write.commit().await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<any_redb_write_tx_ $fn_name>]() -> Result<()> {
                let kv = AnyKV::from(RedbKV::new(None).await?);
                let write = kv.write_tx().await?;
                $fn_name(&write).await?;
                write.commit().await?;
                Ok(())
            }
        }
    };
}

#[macro_export]
macro_rules! all_range_read_impls {
    ($fn_name:ident) => {
        paste::paste! {
            #[tokio::test]
            async fn [<memory_ $fn_name>]() -> Result<()> {
                let kv = MemoryKV::new(None).await?;
                $fn_name(&kv).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<any_memory_ $fn_name>]() -> Result<()> {
                let kv = AnyKV::new(None).await?;
                $fn_name(&kv).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<memory_read_tx_ $fn_name>]() -> Result<()> {
                let kv = MemoryKV::new(None).await?;
                let read = kv.read_tx().await?;
                $fn_name(&read).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<any_memory_read_tx_ $fn_name>]() -> Result<()> {
                let kv = AnyKV::new(None).await?;
                let read = kv.read_tx().await?;
                $fn_name(&read).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<redb_ $fn_name>]() -> Result<()> {
                let kv = RedbKV::new(None).await?;
                $fn_name(&kv).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<any_redb_ $fn_name>]() -> Result<()> {
                let kv = AnyKV::from(RedbKV::new(None).await?);
                $fn_name(&kv).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<redb_read_tx_ $fn_name>]() -> Result<()> {
                let kv = RedbKV::new(None).await?;
                let read = kv.read_tx().await?;
                $fn_name(&read).await?;
                Ok(())
            }

            #[tokio::test]
            async fn [<any_redb_read_tx_ $fn_name>]() -> Result<()> {
                let kv = AnyKV::from(RedbKV::new(None).await?);
                let read = kv.read_tx().await?;
                $fn_name(&read).await?;
                Ok(())
            }
        }
    };
}
