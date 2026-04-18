import asyncio
import httpx
from datetime import datetime, timedelta, timezone
from typing import List, Dict, Any
from questdb.ingress import Sender, IngressError, TimestampNanos

# Binance API settings
BINANCE_API_URL = "https://api.binance.com/api/v3"
LIMIT = 1000

# QuestDB settings
QUESTDB_HOST = "localhost" # Adjust if running docker inside different network
QUESTDB_ILP_PORT = 9009
TABLE_NAME = "klines"

async def fetch_top_pairs(client: httpx.AsyncClient, limit: int = 50) -> List[str]:
    """Fetches the top USDT-margined pairs by 24h trading volume."""
    print("Fetching top pairs by volume...")
    response = await client.get(f"{BINANCE_API_URL}/ticker/24hr")
    response.raise_for_status()
    
    data = response.json()
    usdt_pairs = [item for item in data if item['symbol'].endswith('USDT')]
    
    # Sort by volume descending
    usdt_pairs.sort(key=lambda x: float(x['quoteVolume']), reverse=True)
    
    top_pairs = [item['symbol'] for item in usdt_pairs[:limit]]
    print(f"Top {limit} pairs: {top_pairs}")
    return top_pairs

async def fetch_klines(
    client: httpx.AsyncClient, 
    symbol: str, 
    start_ts: int, 
    end_ts: int, 
    semaphore: asyncio.Semaphore
) -> List[List[Any]]:
    """Fetches klines for a given symbol and time range."""
    async with semaphore:
        params = {
            "symbol": symbol,
            "interval": "1m",
            "startTime": start_ts,
            "endTime": end_ts,
            "limit": LIMIT
        }
        
        # Simple backoff/retry for rate limits
        for attempt in range(3):
            response = await client.get(f"{BINANCE_API_URL}/klines", params=params)
            
            if response.status_code == 429:
                retry_after = int(response.headers.get("Retry-After", 5))
                print(f"Rate limited on {symbol}. Waiting {retry_after}s...")
                await asyncio.sleep(retry_after)
                continue
                
            response.raise_for_status()
            
            # Check weight usage (optional, but good practice)
            weight = int(response.headers.get("X-MBX-USED-WEIGHT-1M", 0))
            if weight > 5000:
                print(f"Warning: High weight usage ({weight}/6000). Throttling...")
                await asyncio.sleep(2)
                
            return response.json()
            
        return []

def insert_into_questdb(sender: Sender, symbol: str, klines: List[List[Any]]):
    """Inserts a batch of klines into QuestDB via ILP."""
    if not klines:
        return
        
    for k in klines:
        # Binance kline format:
        # [Open time, Open, High, Low, Close, Volume, Close time, Quote asset volume, Number of trades, Taker buy base, Taker buy quote, Ignore]
        open_time = TimestampNanos(int(k[0]) * 1_000_000)  # Convert Binance ms to nanos
        
        sender.row(
            TABLE_NAME,
            symbols={"symbol": symbol},
            columns={
                "open": float(k[1]),
                "high": float(k[2]),
                "low": float(k[3]),
                "close": float(k[4]),
                "volume": float(k[5]),
                "quote_asset_volume": float(k[7]),
                "number_of_trades": int(k[8])
            },
            at=open_time
        )
    sender.flush()

async def sync_symbol(
    client: httpx.AsyncClient, 
    symbol: str, 
    start_time: int, 
    end_time: int, 
    semaphore: asyncio.Semaphore,
    sender: Sender
):
    """Syncs historical data for a single symbol in chunks."""
    print(f"Starting sync for {symbol}...")
    current_start = start_time
    total_inserted = 0
    
    while current_start < end_time:
        # Request data from current_start up to the global end_time
        # Binance will skip empty ranges dynamically and return the next valid 1000 candles!
        klines = await fetch_klines(client, symbol, current_start, end_time, semaphore)
        
        if not klines:
            break
            
        insert_into_questdb(sender, symbol, klines)
        total_inserted += len(klines)
        
        # Next start is mathematically 1 minute after the last candle we just received
        current_start = int(klines[-1][0]) + 60 * 1000
        
        # Small sleep to be nice to API
        await asyncio.sleep(0.1)
        
    print(f"Finished {symbol}. Inserted {total_inserted} rows.")

async def main():
    end_datetime = datetime.now(timezone.utc)
    start_datetime = end_datetime - timedelta(days=30)
    
    start_time_ms = int(start_datetime.timestamp() * 1000)
    end_time_ms = int(end_datetime.timestamp() * 1000)
    
    # We use TCP endpoint for ILP (Port 9009)
    conf = f'tcp::addr={QUESTDB_HOST}:{QUESTDB_ILP_PORT};'
    
    try:
        # We use standard HTTP endpoint wrapper for ILP introduced in recent questdb-python versions
        with Sender.from_conf(conf) as sender:
            async with httpx.AsyncClient(timeout=30.0) as client:
                top_pairs = await fetch_top_pairs(client, limit=50)
                
                # Limit concurrent requests to avoid instant 429
                semaphore = asyncio.Semaphore(10) 
                
                tasks = [
                    sync_symbol(client, symbol, start_time_ms, end_time_ms, semaphore, sender)
                    for symbol in top_pairs
                ]
                
                await asyncio.gather(*tasks)
                
        print("Historical sync completed successfully!")
    except IngressError as e:
        print(f"QuestDB Ingress error: {e}")
    except Exception as e:
        print(f"Unexpected error: {e}")

if __name__ == "__main__":
    asyncio.run(main())
