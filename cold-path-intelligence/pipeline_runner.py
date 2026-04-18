import time
import polars as pl
from src.data_ingestion.questdb_client import fetch_closing_prices
from src.discovery.clustering import AssetClusterer
from src.discovery.cointegration import test_cointegration
from src.risk_modeling.kalman_filter import KalmanSpread
from src.risk_modeling.z_score import calculate_z_score

def run_cold_path():
    print("🚀 Starting Stat-Arb Cold-Path Pipeline...")
    start_total = time.time()
    
    # 1. DATA FETCHING
    df = fetch_closing_prices()
    if df.height == 0:
        return
        
    # 2. DISCOVERY (Clustering + Cointegration)
    # Using Cosine metric inside AssetClusterer (as updated previously)
    clusterer = AssetClusterer(n_components=15, min_samples=2)
    clusters = clusterer.fit_predict(df)
    
    if not clusters:
        print("❌ No clusters formed. Exiting.")
        return
        
    coint_df = test_cointegration(df, clusters, p_value_threshold=0.05)
    if coint_df.height == 0:
        print("❌ No cointegrated pairs found. Exiting.")
        return
        
    # 3. RISK MODELING (Kalman + Z-Score)
    print(f"📊 Modeling Risk for {coint_df.height} cointegrated pairs...")
    final_results = []
    
    kalman = KalmanSpread(delta=1e-4, r=1e-3)
    
    for row in coint_df.iter_rows(named=True):
        asset_y = row['asset_1']
        asset_x = row['asset_2']
        
        # Extraction
        pair_data = df.select([asset_y, asset_x]).drop_nulls()
        y = pair_data[asset_y].to_numpy()
        x = pair_data[asset_x].to_numpy()
        
        # Kalman Execution
        try:
            k_res = kalman.estimate_spread(y, x)
            spread = pl.Series(k_res['spread'])
            
            # Z-Score Calculation (using a 24h window = 1440 min)
            z_scores = calculate_z_score(spread, window=1440)
            
            # Extract latest (current) values
            current_z = z_scores.tail(1).item()
            current_beta = k_res['beta'][-1]
            
            final_results.append({
                "pair_y": asset_y,
                "pair_x": asset_x,
                "current_hedge_ratio": current_beta,
                "current_z_score": current_z,
                "entry_threshold": 2.0,
                "exit_threshold": 0.0,
                "stop_loss_threshold": 4.0
            })
        except Exception as e:
            print(f"⚠️ Error modeling pair {asset_y}-{asset_x}: {e}")
            
    # 4. FINAL OUTPUT
    final_df = pl.DataFrame(final_results)
    
    print("\n✅ COLD-PATH COMPLETE")
    print("--- Final Trading Signals ---")
    print(final_df)
    
    end_total = time.time()
    print(f"\nPipeline finished in {end_total - start_total:.2f} seconds.")
    
    return final_df

if __name__ == "__main__":
    run_cold_path()
