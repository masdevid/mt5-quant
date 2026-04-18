# PyInstaller hook for mcp package
# Ensures all mcp submodules are included

from PyInstaller.utils.hooks import collect_submodules, collect_data_files

hiddenimports = collect_submodules('mcp')
datas = collect_data_files('mcp')

# Also ensure anyio dependencies are included
hiddenimports += [
    'anyio',
    'anyio.streams',
    'anyio.streams.memory',
    'anyio.streams.text',
    'anyio._backends',
    'anyio._backends._asyncio',
    'anyio._backends._trio',
]
