use crate::priv_prelude::*;

#[tokio::test(flavor = "multi_thread")]
async fn zero_random_delay_in_order() {
    const NUM_MSGS: usize = 1000;
    const MIN_DELAY: Duration = Duration::from_millis(200);

    let (chan_0, mut chan_1) = BiChannel::new(NUM_MSGS);
    let mut chan_0 = Box::pin(chan_0.with_delay(MIN_DELAY, Duration::ZERO));
    let sender_0 = async move {
        for val in 0..NUM_MSGS {
            chan_0.feed(val).await.unwrap();
        }
        chan_0.flush().await.unwrap();
        chan_0
    };
    let receiver_1 = async move {
        for expected_val in 0..NUM_MSGS {
            let val = chan_1.next().await.unwrap();
            assert_eq!(val, expected_val);
        }
        chan_1
    };
    let sender_0 = tokio::spawn(sender_0);
    let receiver_1 = tokio::spawn(receiver_1);
    let (chan_0_res, chan_1_res) = join!(sender_0, receiver_1);
    let mut chan_0 = chan_0_res.unwrap();
    let mut chan_1 = chan_1_res.unwrap();
    
    let sender_1 = async move {
        for val in 0..NUM_MSGS {
            chan_1.feed(val).await.unwrap();
        }
        chan_1.flush().await.unwrap();
        chan_1
    };
    let receiver_0 = async move {
        for expected_val in 0..NUM_MSGS {
            let val = chan_0.next().await.unwrap();
            assert_eq!(val, expected_val);
        }
        chan_0
    };
    let sender_1 = tokio::spawn(sender_1);
    let receiver_0 = tokio::spawn(receiver_0);
    let (chan_0_res, chan_1_res) = join!(receiver_0, sender_1);
    let mut chan_0 = chan_0_res.unwrap();
    let chan_1 = chan_1_res.unwrap();

    drop(chan_1);
    assert!(chan_0.next().await.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn random_delays_are_approx_correct() {
    const NUM_MSGS: usize = 1_000;
    const MIN_DELAY: Duration = Duration::from_millis(500);
    const MEAN_RANDOM_DELAY: Duration = Duration::from_millis(500);

    let (chan_0, mut chan_1) = BiChannel::new(NUM_MSGS);
    let mut chan_0 = Box::pin(chan_0.with_delay(MIN_DELAY, MEAN_RANDOM_DELAY));

    let check_delays = |delays: Vec<Duration>| {
        let min_delay = delays.iter().copied().min().unwrap();
        assert!(MIN_DELAY <= min_delay);
        assert!(min_delay <= MIN_DELAY * 2);

        let total_random_delays: f64 = {
            delays
            .into_iter()
            .map(|delay| delay - min_delay)
            .map(|random_delay| random_delay.as_secs_f64())
            .sum()
        };
        let mean_random_delay = total_random_delays / (NUM_MSGS as f64);
        let expected_mean_random_delay = MEAN_RANDOM_DELAY.as_secs_f64();
        assert!(expected_mean_random_delay * 0.8 < mean_random_delay);
        assert!(mean_random_delay < expected_mean_random_delay * 1.2);
    };

    let mut delays = Vec::with_capacity(NUM_MSGS);
    let sender = async move {
        for _ in 0..NUM_MSGS {
            let send_instant = Instant::now();
            chan_0.feed(send_instant).await.unwrap();
        }
        chan_0.flush().await.unwrap();
        chan_0
    };
    let receiver = async move {
        for _ in 0..NUM_MSGS {
            let send_instant = chan_1.next().await.unwrap();
            let delay = Instant::now() - send_instant;
            delays.push(delay);
        }
        (chan_1, delays)
    };
    let receiver = tokio::spawn(receiver);
    tokio::time::sleep(MIN_DELAY).await;
    let sender = tokio::spawn(sender);
    let (chan_0_res, chan_1_delays_res) = join!(sender, receiver);
    let mut chan_0 = chan_0_res.unwrap();
    let (mut chan_1, delays) = chan_1_delays_res.unwrap();
    check_delays(delays);

    let mut delays = Vec::with_capacity(NUM_MSGS);
    let sender = async move {
        for _ in 0..NUM_MSGS {
            let send_instant = Instant::now();
            chan_1.feed(send_instant).await.unwrap();
        }
        chan_1.flush().await.unwrap();
        chan_1
    };
    let receiver = async move {
        for _ in 0..NUM_MSGS {
            let send_instant = chan_0.next().await.unwrap();
            let delay = Instant::now() - send_instant;
            delays.push(delay);
        }
        (chan_0, delays)
    };
    let receiver = tokio::spawn(receiver);
    tokio::time::sleep(MIN_DELAY).await;
    let sender = tokio::spawn(sender);
    let (chan_1_res, chan_0_delays_res) = join!(sender, receiver);
    let chan_1 = chan_1_res.unwrap();
    let (chan_0, delays) = chan_0_delays_res.unwrap();
    check_delays(delays);
    
    drop((chan_0, chan_1));
}

