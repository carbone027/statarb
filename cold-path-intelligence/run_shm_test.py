import time
import polars as pl
import flatbuffers

# Classes do FlatBuffers autogeradas no passo anterior
from src.ipc.statarb.ipc.Signal import SignalStart, SignalAddTargetSymbol, SignalAddHedgeSymbol, SignalAddHedgeRatio, SignalAddThreshold, SignalEnd
from src.ipc.statarb.ipc.RiskState import RiskStateStart, RiskStateAddSignals, RiskStateStartSignalsVector, RiskStateAddSequenceNumber, RiskStateAddTimestamp, RiskStateEnd

# Import da nova classe de Shared Memory e do pipeline anterior
from src.ipc.shm_writer import SharedMemoryWriter
from pipeline_runner import run_cold_path

def create_risk_state_buffer(df: pl.DataFrame, seq_number: int) -> bytes:
    """
    Converte o DataFrame Polars resultante das inferências quantitativas num
    array byte (buffer) compactado pelo codec do FlatBuffers.
    """
    # Capacidade inicial para alocação
    builder = flatbuffers.Builder(2048)
    
    signals = []
    
    # 1. Os dados de Strings precisam ser criados antes e os Offsets guardados.
    for row in df.iter_rows(named=True):
        target_sym = builder.CreateString(row['pair_y'])
        hedge_sym = builder.CreateString(row['pair_x'])
        
        SignalStart(builder)
        SignalAddTargetSymbol(builder, target_sym)
        SignalAddHedgeSymbol(builder, hedge_sym)
        SignalAddHedgeRatio(builder, float(row['current_hedge_ratio']))
        SignalAddThreshold(builder, float(row['entry_threshold']))
        # Completa o Sinal e guarda a sua posição relativa em memória
        sig_offset = SignalEnd(builder)
        signals.append(sig_offset)
        
    # 2. Cria o vetor de Signals em memória (em marcha invertida, restrição do FlatBuffers)
    RiskStateStartSignalsVector(builder, len(signals))
    for sig in reversed(signals):
        builder.PrependUOffsetTRelative(sig)
    signals_vec = builder.EndVector()
    
    # 3. Empacota o Objeto Root (RiskState)
    RiskStateStart(builder)
    RiskStateAddSignals(builder, signals_vec)
    RiskStateAddSequenceNumber(builder, seq_number)
    RiskStateAddTimestamp(builder, int(time.time() * 1000))
    rs_offset = RiskStateEnd(builder)
    
    # 4. Finaliza a construção do objeto global
    builder.Finish(rs_offset)
    
    return builder.Output()

def start_continuous_ipc_bridge():
    """
    Roda um loop contínuo de integração entre a Pipeline (geração de modelo quant) e a memória física (IPC).
    """
    print("Iniciando a ponte IPC (Python -> /dev/shm)...")
    
    # Memória alocada de 1MB
    writer = SharedMemoryWriter(name="statarb_signals", size=1024 * 1024)
    tick_counter = 0

    try:
        while True:
            print(f"\n--- [TICK #{tick_counter}] ---")
            df = run_cold_path()
            
            if df is not None and df.height > 0:
                print(f"[IPC] Serializando ({df.height}) pares via FlatBuffers.")
                payload_bytes = create_risk_state_buffer(df, seq_number=tick_counter)
                
                print(f"[IPC] A escrever {len(payload_bytes)} bytes na memória partilhada.")
                writer.write(payload_bytes)
                print("[IPC] Escrita Lock-Free concluída em tempo Zero-Copy.")
                
            else:
                print("Nenhum sinal válido encontrado. Mantendo dados antigos inalterados.")
                
            tick_counter += 1
            
            # Aguarda pela próxima janela avaliativa
            print("A dormir 5 segundos...")
            time.sleep(5)
            
    except KeyboardInterrupt:
        print("\nPipeline Encerrada. Fechando Mmap.")
    except Exception as e:
        print(f"\nErro de execução: {e}")
    finally:
        writer.close()

if __name__ == "__main__":
    start_continuous_ipc_bridge()
