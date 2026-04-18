import polars as pl
import itertools
from statsmodels.tsa.stattools import coint
import warnings

def test_cointegration(df: pl.DataFrame, clusters: dict, p_value_threshold: float = 0.05) -> pl.DataFrame:
    """
    Given a Polars DataFrame of raw prices and a dictionary of clusters,
    generates pairs within each cluster and runs the Engle-Granger test.
    Returns a Polars DataFrame of cointegrated pairs.
    """
    valid_pairs = []
    
    # Ignore warnings from statsmodels converging checks for flat datasets
    warnings.filterwarnings("ignore", category=UserWarning)
    
    total_clusters = len(clusters)
    print(f"Beginning Cointegration testing across {total_clusters} clusters...")
    
    for cluster_id, symbols in clusters.items():
        # Generate O(1) combinations strictly inside the same spatial cluster
        pairs = list(itertools.combinations(symbols, 2))
        print(f"Cluster {cluster_id}: Testing {len(pairs)} internal pairs.")
        
        for asset_1, asset_2 in pairs:
            # We strictly slice and drop any local nulls just for these 2 assets
            pair_df = df.select([asset_1, asset_2]).drop_nulls()
            
            y0 = pair_df[asset_1].to_numpy()
            y1 = pair_df[asset_2].to_numpy()
            
            if len(y0) < 100:
                continue # Skip degenerate lengths
                
            try:
                # Engle-Granger Test
                # maxlag=1 heavily optimizes speed for 43k rows instead of letting it calculate AIC lags.
                score, p_value, _ = coint(y0, y1, maxlag=1)
                
                if p_value < p_value_threshold:
                    valid_pairs.append({
                        "cluster_id": cluster_id,
                        "asset_1": asset_1,
                        "asset_2": asset_2,
                        "p_value": p_value,
                        "t_statistic": score
                    })
            except Exception as e:
                pass # Usually thrown if vectors are perfectly collinear or constants
                
    # Restore warnings
    warnings.filterwarnings("default", category=UserWarning)

    if not valid_pairs:
        print("No cointegrated pairs found passing the p-value threshold!")
        return pl.DataFrame(schema={"cluster_id": pl.Int64, "asset_1": pl.Utf8, "asset_2": pl.Utf8, "p_value": pl.Float64, "t_statistic": pl.Float64})
        
    final_df = pl.DataFrame(valid_pairs).sort("p_value")
    print(f"Discovery phase complete! {final_df.height} pairs detected.")
    
    return final_df
