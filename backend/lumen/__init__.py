import os

__version__ = os.environ.get("LUMEN_VERSION", "") or "Development"
