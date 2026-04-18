import time
from src.data_ingestion.questdb_client import fetch_closing_prices
from src.discovery.clustering import AssetClusterer
from src.discovery.cointegration import test_cointegration

def run_discovery_pipeline():
    start = time.time()
    
    # 1. Fetch pivoted raw time-series data
    df = fetch_closing_prices()
    
    if df.height == 0:
        print("DataFrame is empty. Stopping.")
        return
        
    # Tweak: using cosine distance, PCA features group naturally.
    clusterer = AssetClusterer(n_components=15, min_samples=2)
    clusters = clusterer.fit_predict(df)
    
    if not clusters:
        print("No clusters formed. Consider loosening DBSCAN 'eps' parameters.")
        return
        
    for k, v in clusters.items():
        print(f"  -> Cluster {k}: {v}")
        
    # 3. Test Cointegration within the clustered pairs
    coint_df = test_cointegration(df, clusters, p_value_threshold=0.05)
    
    print("\n--- Top Cointegrated Pairs ---")
    print(coint_df.head(10))
    print(f"\nPipeline finished in {time.time() - start:.2f} seconds.")

if __name__ == "__main__":
    run_discovery_pipeline()
