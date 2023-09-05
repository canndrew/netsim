use crate::priv_prelude::*;

#[tokio::test(flavor = "multi_thread")]
async fn zero_jitter_is_random() {
    const NUM_MSGS: usize = 1000;
    const LOSS_RATE: f64 = 0.3;

    let (chan_0, mut chan_1) = BiChannel::new(NUM_MSGS);
    let mut chan_0 = Box::pin(chan_0.with_loss(LOSS_RATE, Duration::ZERO));
    let sender = async move {
        for val in 0..NUM_MSGS {
            chan_0.feed(val).await.unwrap();
        }
        chan_0.flush().await.unwrap();
        chan_0.close().await.unwrap();
    };
    let receiver = async move {
        let mut received = vec![false; NUM_MSGS];

        while let Some(val) = chan_1.next().await {
            received[val] = true;
        }
        received
    };
    let sender = tokio::spawn(sender);
    let receiver = tokio::spawn(receiver);
    let (sender_res, receiver_res) = join!(sender, receiver);
    let () = sender_res.unwrap();
    let received = receiver_res.unwrap();

    let lost_count = received.iter().filter(|x| !**x).count();
    let loss_rate = (lost_count as f64) / (NUM_MSGS as f64);
    assert!(loss_rate < LOSS_RATE * 1.2);
    assert!(LOSS_RATE < loss_rate * 1.2);

    let mut after_received_count = 0;
    let mut after_received_lost_count = 0;
    for xs in received.windows(2) {
        if xs[0] {
            after_received_count += 1;
            if !xs[1] {
                after_received_lost_count += 1;
            }
        }
    }
    let after_received_loss_rate = (after_received_lost_count as f64) / (after_received_count as f64);
    assert!(after_received_loss_rate < LOSS_RATE * 1.2);
    assert!(LOSS_RATE < after_received_loss_rate * 1.2);
}

#[tokio::test(flavor = "multi_thread")]
async fn non_zero_jitter_is_locally_non_random() {
    const NUM_MSGS: usize = 1000;
    const LOSS_RATE: f64 = 0.3;
    const JITTER_PERIOD: Duration = Duration::from_millis(5);
    const MSG_PERIOD: Duration = Duration::from_millis(1);

    let (chan_0, mut chan_1) = BiChannel::new(NUM_MSGS);
    let mut chan_0 = Box::pin(chan_0.with_loss(LOSS_RATE, JITTER_PERIOD));
    let sender = async move {
        for val in 0..NUM_MSGS {
            chan_0.feed(val).await.unwrap();
            tokio::time::sleep(MSG_PERIOD).await;
        }
        chan_0.flush().await.unwrap();
        chan_0.close().await.unwrap();
    };
    let receiver = async move {
        let mut received = vec![false; NUM_MSGS];

        while let Some(val) = chan_1.next().await {
            received[val] = true;
        }
        received
    };
    let sender = tokio::spawn(sender);
    let receiver = tokio::spawn(receiver);
    let (sender_res, receiver_res) = join!(sender, receiver);
    let () = sender_res.unwrap();
    let received = receiver_res.unwrap();

    let lost_count = received.iter().filter(|x| !**x).count();
    let loss_rate = (lost_count as f64) / (NUM_MSGS as f64);
    assert!(loss_rate < LOSS_RATE * 1.2);
    assert!(LOSS_RATE < loss_rate * 1.2);

    let mut after_received_count = 0;
    let mut after_received_lost_count = 0;
    for xs in received.windows(2) {
        if xs[0] {
            after_received_count += 1;
            if !xs[1] {
                after_received_lost_count += 1;
            }
        }
    }
    let after_received_loss_rate = (after_received_lost_count as f64) / (after_received_count as f64);
    assert!(after_received_loss_rate < LOSS_RATE);
}

