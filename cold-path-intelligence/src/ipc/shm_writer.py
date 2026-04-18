import os
import mmap
import struct

class SharedMemoryWriter:
    """
    Gestor de memória partilhada Zero-Copy utilizando /dev/shm.
    Implementa um mecanismo Seqlock progressivo na cabeça (8 bytes) do buffer.
    """
    def __init__(self, name: str, size: int):
        self.shm_path = f"/dev/shm/{name}"
        self.size = size
        
        # O_CREAT para criar se não existir, O_RDWR para leitura e escrita
        fd = os.open(self.shm_path, os.O_CREAT | os.O_RDWR)
        
        # Garantir o tamanho do ficheiro mapeado em memória
        os.ftruncate(fd, self.size)
        
        # Mapeia na memória (MAP_SHARED para visibilidade entre processos)
        self.mmap = mmap.mmap(fd, self.size, mmap.MAP_SHARED, mmap.PROT_WRITE | mmap.PROT_READ)
        
        # Inicializa o Header (Seqlock) a 0 caso seja ficheiro recém-criado
        current_seq = struct.unpack('<Q', self.mmap[:8])[0]
        if current_seq == 0 or current_seq % 2 != 0:
            # Se for impar, é porque ficou inconsistente num crash anterior, dá reset a 0.
            self.mmap[:8] = struct.pack('<Q', 0)

    def write(self, payload: bytes):
        """
        Escreve o payload na memória partilhada utilizando um Seqlock.
        """
        payload_len = len(payload)
        # Header (8) + Tamanho do payload não podem exceder a memória mapeada
        if 8 + payload_len > self.size:
            raise ValueError(f"Payload de tamanho {payload_len} excede a shm (Máx {self.size - 8})")
            
        # 1. Obter o Sequence Number (Header) atual
        current_seq = struct.unpack('<Q', self.mmap[:8])[0]
        
        # 2. Incrementar para um IMBAR (Sinalizar inicio de escrita, inviabilizando leitura parcial ao Rust)
        next_seq = current_seq + 1
        if next_seq % 2 == 0:
            next_seq += 1
            
        # Em x86 as store queues evitam explicit memory barriers em Python para threads normais,
        # mas escrevemos o header explicitamente antes da payload.
        self.mmap[:8] = struct.pack('<Q', next_seq)
        
        # 3. Inserir o Payload por inteiro (FlatBuffers)
        self.mmap[8:8 + payload_len] = payload
        
        # 4. Incrementar para um PAR (Sinalizar fim da escrita - o Rust já pode validar)
        final_seq = next_seq + 1
        self.mmap[:8] = struct.pack('<Q', final_seq)

    def close(self):
        """Fecha o descritor mmap"""
        self.mmap.close()
