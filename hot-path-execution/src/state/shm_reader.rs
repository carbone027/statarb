use memmap2::MmapOptions;
use std::fs::OpenOptions;
use std::sync::atomic::{compiler_fence, Ordering};
use tracing::{warn, error};

use crate::engine::router::RiskMatrix;
use crate::ipc::risk_matrix_generated::statarb::ipc::root_as_risk_state;

pub struct SharedMemoryReader {
    mmap: memmap2::Mmap,
}

impl SharedMemoryReader {
    pub fn new(name: &str) -> anyhow::Result<Self> {
        let path = format!("/dev/shm/{}", name);
        let file = OpenOptions::new().read(true).write(true).open(&path)?;

        // Safety: Unsafe since external processes can mock with memory, but we use Seqlocks.
        let mmap = unsafe { MmapOptions::new().map(&file)? };

        Ok(Self { mmap })
    }

    pub fn read_signals(&self) -> Option<Vec<RiskMatrix>> {
        let mut loop_count = 0;

        loop {
            loop_count += 1;
            if loop_count > 10000 {
                warn!("[SHM] Seqlock em deadlock (Python writer travado?). Frame ignorado.");
                return None;
            }

            if self.mmap.len() < 8 {
                return None;
            }

            // 1. Snapshot do Header
            let mut header = [0u8; 8];
            header.copy_from_slice(&self.mmap[0..8]);
            let seq1 = u64::from_le_bytes(header);

            // Ímpar = Writer a escrever. Spin wait until Par.
            if seq1 % 2 != 0 {
                std::hint::spin_loop();
                continue;
            }

            // Memory Fence (Prevenir reordenação otimizada do CPU antes de ler o payload)
            compiler_fence(Ordering::Acquire);

            // 2. Deserializar FlatBuffers Zero-Copy
            let payload_slice = &self.mmap[8..];
            let mut extracted = Vec::new();
            let mut valid_parse = true;

            match root_as_risk_state(payload_slice) {
                Ok(risk_state) => {
                    if let Some(signals) = risk_state.signals() {
                        for sig in signals {
                            let t_sym = sig.target_symbol().unwrap_or("").to_string();
                            let h_sym = sig.hedge_symbol().unwrap_or("").to_string();
                            let ratio = sig.hedge_ratio();
                            let thresh = sig.threshold();

                            extracted.push(RiskMatrix {
                                target_symbol: t_sym,
                                hedge_symbol: h_sym,
                                hedge_ratio: ratio,
                                threshold: thresh,
                            });
                        }
                    }
                }
                Err(_e) => {
                    // Frame incompleto capturado entre bytes (falhou checksum flatbuffers)
                    valid_parse = false;
                }
            }

            // Memory Fence para terminar leitura
            compiler_fence(Ordering::Release);

            // 3. Snapshot 2 do Header para validação (Seqlock)
            let mut header2 = [0u8; 8];
            header2.copy_from_slice(&self.mmap[0..8]);
            let seq2 = u64::from_le_bytes(header2);

            if seq1 == seq2 {
                if valid_parse {
                    return Some(extracted);
                } else {
                    return None;
                }
            } else {
                // Estado corrompido: O Python alterou o payload enquanto o Rust estava lendo.
                // Re-tentar a extração descartando este buffer contaminado.
                std::hint::spin_loop();
                continue;
            }
        }
    }
}
