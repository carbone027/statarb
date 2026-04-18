import polars as pl
import urllib.parse

# QuestDB REST CSV Export Endpoint
QUESTDB_URI = "http://localhost:9000/exp"

def fetch_closing_prices() -> pl.DataFrame:
    """
    Fetches the `close` prices for all assets stored in QuestDB and 
    performs a highly-optimized in-memory pivot using Polars multithreading.
    
    Returns:
        pl.DataFrame: A DataFrame indexed by `timestamp` with each column 
                      representing a symbol's closing price.
    """
    # 1. Query the raw granular data. We use the /exp REST endpoint directly
    # because QuestDB Postgres Wire protocol (ADBC) struggles with some `pg_catalog` queries.
    query = "SELECT timestamp, symbol, close FROM klines ORDER BY timestamp ASC"
    
    print("Executing query via QuestDB REST endpoint...")
    
    # URL Encode the SQL query
    url = f"{QUESTDB_URI}?query={urllib.parse.quote(query)}"
    
    # Polars read_csv can natively stream data from HTTP, taking full advantage of Rust parallel parsing!
    df = pl.read_csv(url, try_parse_dates=True)
    
    if df.height == 0:
        print("Warning: Retrieved empty data. Has the ingestion script been run?")
        return df

    print(f"Successfully fetched {df.height} rows. Performing pivot...")
    
    # Ensure there are no duplicate timestamps for the same symbol (e.g. from restarted syncs)
    df = df.unique(subset=["timestamp", "symbol"], keep="last")
    
    # 2. Perform the analytical PIVOT in Polars. 
    # Polars pivot is incredibly performant and handles dynamic number of columns automatically.
    pivoted_df = df.pivot(
        index="timestamp",
        on="symbol",
        values="close"
    )
    
    print("Pivot completed successfully.")
    return pivoted_df

if __name__ == "__main__":
    import time
    start = time.time()
    try:
        final_df = fetch_closing_prices()
        print(f"Process completed in {time.time() - start:.2f} seconds.")
        print(final_df.head())
        print(f"Shape: {final_df.shape}")
    except Exception as e:
        print(f"Failed to fetch and pivot data: {e}. Is QuestDB running?")
