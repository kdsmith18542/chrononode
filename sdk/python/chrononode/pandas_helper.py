try:
    import pandas as pd
except ImportError:
    pd = None

def to_dataframe(data: list, category: str = "blocks") -> "pd.DataFrame":
    """Converts a list of dicts from ChronoNode queries into a Pandas DataFrame.
    
    Args:
        data: A list of dicts representing blocks, transactions, or events.
        category: The category of data (blocks/txs/events) for potential future custom flattening.
        
    Returns:
        A pandas.DataFrame object.
    
    Raises:
        ImportError: If pandas is not installed.
    """
    if pd is None:
        raise ImportError(
            "The 'pandas' library is not installed. "
            "Please install it via 'pip install pandas' or 'pip install \".[pandas]\"' to use this utility."
        )
    return pd.DataFrame(data)
