#![cfg(feature = "test-util")]

use std::{panic::AssertUnwindSafe, time::Duration};

use futures::FutureExt;

use elfo::prelude::*;

#[message]
struct Terminate;

#[message]
struct Terminated;

#[tokio::test]
async fn it_restarts_explicitly() {
    let schema = ActorGroup::new().exec(move |mut ctx| async move {
        while let Some(envelope) = ctx.recv().await {
            msg!(match envelope {
                Terminate { .. } => break,
                _ => unreachable!(),
            });
        }
        ctx.send(Terminated).await.unwrap();
    });

    let mut proxy = elfo::test::proxy(schema, elfo::config::AnyConfig::default()).await;

    for _ in 1..5 {
        proxy.send(Terminate).await;
        assert_msg!(proxy.recv().await, Terminated);
    }
}

#[tokio::test]
async fn it_restarts_with_timeout_after_failures() {
    tokio::time::pause();

    let schema = ActorGroup::new().exec(move |mut ctx| async move {
        while let Some(envelope) = ctx.recv().await {
            msg!(match envelope {
                Terminate { .. } => panic!("boom!"),
                _ => unreachable!(),
            });
        }
    });

    let mut proxy = elfo::test::proxy(schema, elfo::config::AnyConfig::default()).await;

    for i in 1..5 {
        proxy.send(Terminate).await;

        let r = AssertUnwindSafe(async { proxy.recv().await })
            .catch_unwind()
            .await;
        assert!(r.is_err());

        // https://github.com/tokio-rs/tokio/issues/3985
        tokio::time::sleep(Duration::from_millis(5000 * i + 1)).await;
    }
}
