import polars as pl

def calculate_z_score(spread_series: pl.Series, window: int = 1440) -> pl.Series:
    """
    Calculates the Z-Score of a given spread series using a rolling window.
    Default window is 1440 (1 day if 1-minute data).
    
    Args:
        spread_series (pl.Series): The residual spread from Kalman.
        window (int): Lookback window for rolling mean and std.
        
    Returns:
        pl.Series: The Z-Score series.
    """
    # Force float for calculations
    s = spread_series.cast(pl.Float64)
    
    rolling_mean = s.rolling_mean(window_size=window)
    rolling_std = s.rolling_std(window_size=window)
    
    z_score = (s - rolling_mean) / rolling_std
    
    return z_score
