#!/bin/bash

# Configuration
SCHEMA_DIR="./schemas"
SCHEMA_FILE="$SCHEMA_DIR/risk_matrix.fbs"

# Output directories for the sub-projects
PYTHON_OUT_DIR="../cold-path-intelligence/src/ipc"
RUST_OUT_DIR="../hot-path-execution/src/ipc"

# Create output directories if they don't exist
mkdir -p "$PYTHON_OUT_DIR"
mkdir -p "$RUST_OUT_DIR"

# Print banner
echo "=================================================="
echo " Building FlatBuffers IPC Schema "
echo "=================================================="

# Check if flatc is installed
if ! command -v flatc &> /dev/null
then
    echo "[ERRO] 'flatc' não foi encontrado."
    echo "Por favor instale o compilador FlatBuffers."
    exit 1
fi

# Compile for Python 3.12 (Cold Layer)
echo "[INFO] A compilar modelo para Python (Cold Layer)..."
flatc --python -o "$PYTHON_OUT_DIR" "$SCHEMA_FILE"
if [ $? -eq 0 ]; then
    echo "  > Sucesso! Bindings Python guardados em: $PYTHON_OUT_DIR"
else
    echo "  > [ERRO] Falha ao compilar para Python."
fi

# Compile for Rust 2021 (Hot Layer)
echo "[INFO] A compilar modelo para Rust (Hot Layer)..."
flatc --rust -o "$RUST_OUT_DIR" "$SCHEMA_FILE"
if [ $? -eq 0 ]; then
    echo "  > Sucesso! Bindings Rust guardados em: $RUST_OUT_DIR"
else
    echo "  > [ERRO] Falha ao compilar para Rust."
fi

echo "=================================================="
echo " Compilação finalizada!"
echo "=================================================="
