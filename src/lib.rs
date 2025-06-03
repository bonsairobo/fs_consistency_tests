#[cfg(test)]
mod tests {
    use smol::unblock;
    use std::path::{Path, PathBuf};

    #[smol_potat::test]
    async fn async_fs_read_after_write_consistency() {
        use futures_lite::{AsyncReadExt, AsyncWriteExt};
        use futures_util::StreamExt;

        let tmpdir = tempfile::tempdir().unwrap();
        let (path_tx, path_rx) = async_channel::bounded(16);
        let writer_dir = tmpdir.path().to_owned();
        let write_task = smol::spawn(async move {
            let writer_dir_ref = &writer_dir;
            let path_tx_ref = &path_tx;
            let bytes = b"hello".as_slice();
            futures_util::stream::iter(0..400_000)
                .for_each_concurrent(4, |i| async move {
                    let path = writer_dir_ref.join(i.to_string());
                    let mut f = async_fs_create_clobber_options().open(&path).await.unwrap();
                    f.write_all(bytes).await.unwrap();
                    // f.sync_data().await.unwrap();
                    path_tx_ref.send(path).await.unwrap();
                })
                .await;
        });
        let read_task = smol::spawn(async move {
            let bytes = b"hello".as_slice();
            while let Ok(path) = path_rx.recv().await {
                let mut f = async_fs::File::open(&path).await.unwrap();
                let mut buf = Vec::with_capacity(5);
                f.read_to_end(&mut buf).await.unwrap();
                assert_eq!(bytes, buf, "{path:?}");
                async_fs::remove_file(&path).await.unwrap();
            }
        });
        write_task.await;
        read_task.await;
    }

    #[smol_potat::test]
    async fn unblock_std_fs_read_after_write_consistency() {
        use futures_util::StreamExt;
        use std::io::{Read, Write};

        let tmpdir = tempfile::tempdir().unwrap();
        let (path_tx, path_rx) = async_channel::bounded(16);
        let writer_dir = tmpdir.path().to_owned();
        let write_task = smol::spawn(async move {
            let writer_dir_ref = &writer_dir;
            let path_tx_ref = &path_tx;
            let bytes = b"hello".as_slice();
            futures_util::stream::iter(0..400_000)
                .for_each_concurrent(4, |i| async move {
                    let writer_dir = writer_dir_ref.clone();
                    let path_tx = path_tx_ref.clone();
                    unblock(move || {
                        let path = writer_dir.join(i.to_string());
                        let mut f = std_fs_create_clobber_options().open(&path).unwrap();
                        f.write_all(bytes).unwrap();
                        // f.sync_data().unwrap();
                        path_tx.send_blocking(path).unwrap();
                    })
                    .await;
                })
                .await;
        });
        let read_task = smol::spawn(async move {
            let bytes = b"hello".as_slice();
            while let Ok(path) = path_rx.recv().await {
                unblock(move || {
                    let mut f = std::fs::File::open(&path).unwrap();
                    let mut buf = Vec::with_capacity(5);
                    f.read_to_end(&mut buf).unwrap();
                    assert_eq!(bytes, buf, "{path:?}");
                    std::fs::remove_file(path).unwrap();
                })
                .await;
            }
        });
        write_task.await;
        read_task.await;
    }

    #[tokio::test]
    async fn tokio_fs_read_after_write_consistency() {
        use futures_util::StreamExt;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let tmpdir = tempfile::tempdir().unwrap();
        let (path_tx, path_rx) = async_channel::bounded(16);
        let writer_dir = tmpdir.path().to_owned();
        let write_task = tokio::spawn(async move {
            let writer_dir_ref = &writer_dir;
            let path_tx_ref = &path_tx;
            let bytes = b"hello".as_slice();
            futures_util::stream::iter(0..40_000)
                .for_each_concurrent(4, |i| async move {
                    let path = writer_dir_ref.join(i.to_string());
                    let mut f = tokio_fs_create_clobber_options().open(&path).await.unwrap();
                    f.write_all(bytes).await.unwrap();
                    // f.sync_data().await.unwrap();
                    path_tx_ref.send(path).await.unwrap();
                })
                .await;
        });
        let read_task = tokio::spawn(async move {
            let bytes = b"hello".as_slice();
            while let Ok(path) = path_rx.recv().await {
                let mut f = tokio::fs::File::open(&path).await.unwrap();
                let mut buf = Vec::with_capacity(5);
                f.read_to_end(&mut buf).await.unwrap();
                assert_eq!(bytes, buf, "{path:?}");
                tokio::fs::remove_file(path).await.unwrap();
            }
        });
        write_task.await.unwrap();
        read_task.await.unwrap();
    }

    #[test]
    fn std_fs_read_after_write_consistency() {
        use std::io::Read;

        let tmpdir = tempfile::tempdir().unwrap();
        let (path_tx, path_rx) = async_channel::bounded(16);
        let writer_dir = tmpdir.path();
        let bytes = b"hello".as_slice();
        let path_tx_ref = &path_tx;
        std::thread::scope(|scope| {
            scope.spawn(move || writer_thread(writer_dir, 0, 10_000, path_tx_ref.clone()));
            scope.spawn(move || writer_thread(writer_dir, 10_000, 20_000, path_tx_ref.clone()));
            scope.spawn(move || writer_thread(writer_dir, 20_000, 30_000, path_tx_ref.clone()));
            scope.spawn(move || writer_thread(writer_dir, 30_000, 40_000, path_tx_ref.clone()));

            scope.spawn(move || {
                for _ in 0..40_000 {
                    let path = path_rx.recv_blocking().unwrap();
                    let mut f = std::fs::File::open(&path).unwrap();
                    let mut buf = Vec::with_capacity(5);
                    f.read_to_end(&mut buf).unwrap();
                    assert_eq!(bytes, buf, "{path:?}");
                    std::fs::remove_file(path).unwrap();
                }
            });
        });
    }
    fn writer_thread(
        dir: &Path,
        start: usize,
        end: usize,
        path_tx: async_channel::Sender<PathBuf>,
    ) {
        use std::io::Write;

        let bytes = b"hello".as_slice();
        for i in start..end {
            let path = dir.join(i.to_string());
            let mut f = std_fs_create_clobber_options().open(&path).unwrap();
            f.write_all(bytes).unwrap();
            // f.sync_data().unwrap();
            path_tx.send_blocking(path).unwrap();
        }
    }

    #[smol_potat::test]
    async fn async_fs_read_after_unblock_std_fs_write_consistency() {
        use futures_lite::AsyncReadExt;
        use futures_util::StreamExt;
        use std::io::Write;

        let tmpdir = tempfile::tempdir().unwrap();
        let (path_tx, path_rx) = async_channel::bounded(16);
        let writer_dir = tmpdir.path().to_owned();
        let write_task = smol::spawn(async move {
            let writer_dir_ref = &writer_dir;
            let path_tx_ref = &path_tx;
            let bytes = b"hello".as_slice();
            futures_util::stream::iter(0..4_000_000)
                .for_each_concurrent(4, |i| async move {
                    let writer_dir = writer_dir_ref.clone();
                    let path_tx = path_tx_ref.clone();
                    unblock(move || {
                        let path = writer_dir.join(i.to_string());
                        let mut f = std_fs_create_clobber_options().open(&path).unwrap();
                        f.write_all(bytes).unwrap();
                        // f.sync_data().unwrap();
                        path_tx.send_blocking(path).unwrap();
                    })
                    .await;
                })
                .await;
        });
        let read_task = smol::spawn(async move {
            let bytes = b"hello".as_slice();
            while let Ok(path) = path_rx.recv().await {
                let mut f = async_fs::File::open(&path).await.unwrap();
                let mut buf = Vec::with_capacity(5);
                f.read_to_end(&mut buf).await.unwrap();
                assert_eq!(bytes, buf, "{path:?}");
                std::fs::remove_file(&path).unwrap();
            }
        });
        write_task.await;
        read_task.await;
    }

    fn std_fs_create_clobber_options() -> std::fs::OpenOptions {
        let mut opts = std::fs::OpenOptions::new();
        opts.read(true).write(true).create(true);
        opts
    }
    fn async_fs_create_clobber_options() -> async_fs::OpenOptions {
        let mut opts = async_fs::OpenOptions::new();
        opts.read(true).write(true).create(true);
        opts
    }
    fn tokio_fs_create_clobber_options() -> tokio::fs::OpenOptions {
        let mut opts = tokio::fs::OpenOptions::new();
        opts.read(true).write(true).create(true);
        opts
    }
}
