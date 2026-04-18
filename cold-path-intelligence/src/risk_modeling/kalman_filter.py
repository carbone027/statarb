import numpy as np
from pykalman import KalmanFilter
import polars as pl

class KalmanSpread:
    def __init__(self, delta: float = 1e-5, r: float = 0.001):
        """
        Initializes the Kalman Filter for dynamic regression: y = beta * x + alpha.
        
        Args:
            delta (float): Transition covariance (process noise). Controls how fast beta/alpha adapt.
            r (float): Observation covariance (measurement noise).
        """
        self.delta = delta
        self.r = r

    def estimate_spread(self, y: np.ndarray, x: np.ndarray):
        """
        Estimates the dynamic hedge ratio (beta) and intercept (alpha).
        
        Args:
            y (np.ndarray): Price series of Asset Y.
            x (np.ndarray): Price series of Asset X.
            
        Returns:
            dict: {
                'spread': series of residual spread,
                'beta': dynamic hedge ratio series,
                'alpha': dynamic intercept series
            }
        """
        # Observation matrix H: [x_t, 1] for y_t = [x_t, 1] * [beta_t, alpha_t]^T
        # We need to reshape for pykalman: (n_observations, n_measurements, n_states)
        obs_mat = np.vstack([x, np.ones(len(x))]).T[:, np.newaxis, :]
        
        # Transition matrix (Identity since we assume beta/alpha follow a random walk)
        trans_mat = np.eye(2)
        
        # Process noise: how much the state changes between steps
        trans_cov = self.delta / (1 - self.delta) * np.eye(2)
        
        kf = KalmanFilter(
            n_dim_obs=1, 
            n_dim_state=2,
            initial_state_mean=np.zeros(2),
            initial_state_covariance=np.ones((2, 2)),
            transition_matrices=trans_mat,
            observation_matrices=obs_mat,
            observation_covariance=self.r,
            transition_covariance=trans_cov
        )
        
        state_means, _ = kf.filter(y)
        
        betas = state_means[:, 0]
        alphas = state_means[:, 1]
        
        # Spread = Y - (Beta * X + Alpha)
        spread = y - (betas * x + alphas)
        
        return {
            'spread': spread,
            'beta': betas,
            'alpha': alphas
        }
