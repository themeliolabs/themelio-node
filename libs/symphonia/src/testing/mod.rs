use crate::{Config, Decider, Machine, Pacemaker};
use smol::lock::Mutex;
use smol::prelude::*;
use std::{collections::BTreeMap, collections::HashMap, sync::Arc, time::SystemTime};
use tmelcrypt::Ed25519PK;

mod unreliable;

/// A harness for testing that uses a mock network to transport messages. Uses a builder-style pattern and should be "run" at the end.
pub struct Harness {
    network: MockNet,
    participants: Vec<(tmelcrypt::Ed25519SK, u64)>,
}

impl Harness {
    /// Create a new harness with the given network config.
    pub fn new(network: MockNet) -> Self {
        Harness {
            network,
            participants: Vec::new(),
        }
    }
    /// Adds a new participant, represented by a secret key and a vote weight.
    pub fn add_participant(mut self, sk: tmelcrypt::Ed25519SK, weight: u64) -> Self {
        self.participants.push((sk, weight));
        self
    }
    /// Runs the harness until all honest participants decide.
    pub async fn run(self, _: MetricsGatherer) {
        let metrics_gatherer = Arc::new(MetricsGatherer::new());
        let (send_global, recv_global) = unreliable::unbounded(self.network);
        let num_participants = self.participants.len();
        let total_weight: u64 = self.participants.iter().map(|(_, w)| w).sum();
        let weight_map: HashMap<tmelcrypt::Ed25519PK, f64> = self
            .participants
            .iter()
            .map(|(sk, w)| (sk.to_public(), *w as f64 / total_weight as f64))
            .collect();
        let sender_weight = Arc::new(move |pk: tmelcrypt::Ed25519PK| {
            weight_map.get(&pk).cloned().unwrap_or_default()
        });
        let pp = self.participants.clone();
        let view_leader = Arc::new(move |view: u64| pp[view as usize % pp.len()].0.to_public());
        let is_valid_prop = Arc::new(|prop: &[u8]| prop[0] % 2 == 0);
        let gen_proposal = Arc::new(|| String::from("nuuunuuNUUU!").as_bytes().to_vec());
        let mut pacemakers = HashMap::new();
        let (send_decision, recv_decision) = smol::channel::unbounded();
        for (sk, _) in self.participants {
            let pk = sk.to_public();
            let cfg = Config {
                sender_weight: sender_weight.clone(),
                view_leader: view_leader.clone(),
                is_valid_prop: is_valid_prop.clone(),
                gen_proposal: gen_proposal.clone(),
                my_sk: sk,
                my_pk: pk.clone(),
            };
            let machine = Machine::new(cfg);
            let pmaker = Arc::new(Pacemaker::new(machine));
            // output waiter
            let fut_out_wait = {
                let pmaker = pmaker.clone();
                let send_global = send_global.clone();
                let send_metrics = metrics_gatherer.clone();
                async move {
                    loop {
                        // Get output from pacemaker and send to global channel
                        let output = pmaker.next_output().await;
                        let _ = send_global.send(output.clone()).await;
                        // Store event in metrics gatherer
                        let (dest, signed_msg) = output.clone();
                        if let Some(d) = dest {
                            send_metrics
                                .store(Event::Sent {
                                    sender: signed_msg.msg.sender,
                                    destination: d,
                                })
                                .await;
                        }
                    }
                }
            };
            // decision waiter
            smolscale::spawn(
                {
                    let pmaker = pmaker.clone();
                    let send_decision = send_decision.clone();
                    let decider_pk = pk.clone();
                    let decision_metrics = metrics_gatherer.clone();
                    async move {
                        let decision = pmaker.decision().await;
                        send_decision.try_send(decision).unwrap();
                        // Store decision event
                        decision_metrics
                            .store(Event::Decided {
                                participant: decider_pk,
                            })
                            .await;
                    }
                }
                .or(fut_out_wait),
            )
            .detach();
            pacemakers.insert(sk.to_public(), pmaker);
        }
        // message stuffer, drop automatically
        let _stuffer = smolscale::spawn(async move {
            loop {
                let (dest, signed_msg) = recv_global.recv().await.unwrap();
                if let Some(dest) = dest {
                    // store received event
                    // let mut events = metrics_gatherer.clone().try_write().unwrap();
                    // events.insert(
                    //     SystemTime::now(),
                    //     Event::Received {
                    //         sender: signed_msg.msg.sender,
                    //         destination: dest,
                    //     },
                    // );

                    // there's a definite destination
                    let dest = pacemakers.get(&dest).expect("nonexistent destination");
                    dest.process_input(signed_msg);
                } else {
                    for (_, dest) in pacemakers.iter() {
                        dest.process_input(signed_msg.clone());
                    }
                }
            }
        });
        // time to wait for the decisions
        for _ in 0..num_participants {
            let dec = recv_decision.recv().await.unwrap();
            dbg!(dec);
        }
    }
}

/// A mock-network.
#[derive(Clone, Debug, Copy)]
pub struct MockNet {
    pub latency_mean_ms: f64,
    pub latency_standard_deviation: f64,
    pub loss_prob: f64,
}

/// An efficient lossy channel.
///
/// Elements can be stuffed in, and they will be delayed until a given time or lost before they can be read out. This simulates a bad network connection or other similar construct.
pub struct LossyChan;

/// Consensus state event data
///
pub enum Event {
    Sent {
        sender: Ed25519PK,
        destination: Ed25519PK,
    },
    Received {
        sender: Ed25519PK,
        destination: Ed25519PK,
    },
    Decided {
        participant: Ed25519PK,
    },
}

pub struct MetricsGatherer {
    pub synced_map: Mutex<BTreeMap<SystemTime, Event>>,
}

impl MetricsGatherer {
    pub fn new() -> MetricsGatherer {
        return MetricsGatherer {
            synced_map: Mutex::new(BTreeMap::new()),
        };
    }
    pub async fn store(&self, event: Event) {
        let mut map = self.synced_map.lock().await;
        map.insert(SystemTime::now(), event);
    }
}
