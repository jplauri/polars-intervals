import polars as pl

INTEGER_DTYPES = [pl.Int8, pl.Int16, pl.Int32, pl.Int64, pl.UInt8, pl.UInt16, pl.UInt32, pl.UInt64]
ENDPOINT_DTYPES = [
    *INTEGER_DTYPES,
    pl.Date,
    *(
        pl.Datetime(unit, zone)
        for unit in ("ms", "us", "ns")
        for zone in (None, "UTC", "Europe/Helsinki")
    ),
]
