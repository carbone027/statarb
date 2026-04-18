import polars as pl
from sklearn.decomposition import PCA
from sklearn.cluster import DBSCAN
import numpy as np

class AssetClusterer:
    def __init__(self, n_components: int = 10, eps: float = 0.5, min_samples: int = 2):
        """
        Initializes the Clustering engine using PCA dimensionality reduction 
        and DBSCAN for spatial grouping of highly correlated asset paths.
        """
        self.n_components = n_components
        self.eps = eps
        self.min_samples = min_samples

    def fit_predict(self, df: pl.DataFrame) -> dict:
        """
        Takes a pivot Polars DataFrame (index: timestamp, cols: assets).
        Applies log returns, z-score normalization, PCA, and DBSCAN.
        Returns a dictionary grouping cluster_ids with their respective coin lists.
        """
        print(f"Starting Clustering Engine for {df.width - 1} assets...")
        
        # 1. Extract symbols 
        symbols = [c for c in df.columns if c != "timestamp"]
        
        # 2. Vectorized Log Returns: ln(P_t) - ln(P_{t-1})
        print("Calculating log-returns...")
        exprs = []
        for s in symbols:
            # Using log1p(pct_change) is a mathematically sound way to do log returns in Polars
            # Alternatively: (col(s) / col(s).shift(1)).log()
            exprs.append((pl.col(s) / pl.col(s).shift(1)).log().alias(s))
            
        returns_df = df.select(exprs).drop_nulls()
        
        # 3. Z-Score Normalization
        # Crucial step so volatile assets (e.g. PEPE) don't dominate PCA distances over stable ones
        print("Standardizing returns...")
        z_exprs = []
        for s in symbols:
            z_exprs.append(((pl.col(s) - pl.col(s).mean()) / pl.col(s).std()).alias(s))
            
        norm_returns_df = returns_df.select(z_exprs)
        
        # 4. Transpose logic for Scikit-Learn
        # Features should be the timestamps (samples sequence), and instances should be the Assets.
        # Shape becomes: (Assets, Timestamps)
        X = norm_returns_df.to_numpy().T 
        
        # 5. Dimensionality Reduction (PCA)
        # Prevents the "curse of dimensionality" when pushing 43000 timestamps into DBSCAN distances
        n_comp = min(self.n_components, len(symbols), X.shape[1])
        print(f"Applying PCA reduction to {n_comp} components...")
        
        pca = PCA(n_components=n_comp)
        pca_features = pca.fit_transform(X)
        
        print(f"PCA Total Variance Explained: {pca.explained_variance_ratio_.sum() * 100:.2f}%")
        
        # 6. Cluster via DBSCAN using Cosine Distance
        print("Running DBSCAN algorithm (cosine metric)...")
        # cosine distance = 1 - cosine similarity. 
        # eps=0.05 means we require > 95% similarity in the PCA space.
        dbscan = DBSCAN(eps=0.05, min_samples=self.min_samples, metric="cosine")
        labels = dbscan.fit_predict(pca_features)
        
        # 7. Map clusters
        clusters = {}
        for symbol, label in zip(symbols, labels):
            if label == -1:
                # -1 means the asset is erratic/noisy and doesn't belong broadly to any dense path
                continue
            clusters.setdefault(label, []).append(symbol)
            
        # Filter strictly clusters that actually act as pairs (len > 1)
        clusters = {k: v for k, v in clusters.items() if len(v) > 1}
        
        total_paired = sum(len(v) for v in clusters.values())
        print(f"Clustering finished: {total_paired} assets assigned into {len(clusters)} valid clusters.")
        
        return clusters
