# Cold-Path Intelligence

Market-Neutral Statistical Arbitrage Cold-Path Intelligence

## Setup

1. `docker-compose up -d` to start QuestDB.
2. `uv run python src/data_ingestion/historical_sync.py` to ingest history.
3. `uv run python src/data_ingestion/questdb_client.py` to fetch and pivot.
