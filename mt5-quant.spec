# -*- mode: python ; coding: utf-8 -*-
"""
PyInstaller spec file for MT5-Quant MCP Server

Build:
  pyinstaller mt5-quant.spec

Output:
  dist/mt5-quant (single executable)
"""

import os
import sys
from pathlib import Path
from PyInstaller.utils.hooks import collect_all

block_cipher = None

# Project root - the build script runs from project root, so use cwd
root = Path(os.getcwd()).resolve()

# Collect entire mcp package (datas, binaries, hiddenimports)
mcp_datas, mcp_binaries, mcp_hiddenimports = collect_all('mcp')

# Data files to include
# Format: (source_path, dest_dir_in_bundle)
datas = [
    # Include scripts directory (bash pipeline scripts)
    (str(root / 'scripts'), 'scripts'),
    # Include docs if needed
    (str(root / 'docs'), 'docs'),
] + mcp_datas

# Hidden imports (modules that are imported dynamically or might be missed)
# Note: mcp_hiddenimports already includes all mcp submodules from collect_all()
hiddenimports = mcp_hiddenimports + [
    # MCP package - all submodules
    'mcp',
    'mcp.types',
    'mcp.server',
    'mcp.server.stdio',
    'mcp.server.models',
    'mcp.server.session',
    'mcp.shared',
    'mcp.shared.memory',
    'mcp.shared.session',
    'mcp.shared.exceptions',
    'mcp.shared.context',
    'mcp.shared.progress',
    'mcp.shared.version',
    'mcp.client',
    'mcp.cli',
    # Analytics modules (local project modules)
    'analytics',
    'analytics.extract',
    'analytics.analyze', 
    'analytics.optimize_parser',
    # Other dependencies
    'pydantic',
    'pydantic.v1',
    'pydantic.v1.fields',
    'pydantic.v1.main',
    'pydantic_core',
    'yaml',
    '_yaml',
    'yaml.constructor',
    'yaml.representer',
    'yaml.cyaml',
    'anyio',
    'anyio.streams',
    'anyio.streams.memory',
    'anyio.streams.text',
    'anyio._backends',
    'anyio._backends._asyncio',
    # Other stdlib that might be dynamically imported
    'xml.etree.ElementTree',
    'html.parser',
    'select',
    'ssl',
    '_ssl',
    'certifi',
    'email',
    'email.parser',
    'email.message',
    'importlib.metadata',
]

a = Analysis(
    ['server/main.py'],
    pathex=[str(root), str(root / 'server'), str(root / 'analytics')],
    binaries=mcp_binaries,
    datas=datas,
    hiddenimports=hiddenimports,
    hookspath=[str(root / 'hooks')],
    hooksconfig={
        'mcp': {
            'include_all': True,
        }
    },
    runtime_hooks=[],
    excludes=[
        # Exclude unnecessary packages to reduce binary size
        'matplotlib',
        'PIL',
        'numpy',
        'pandas',
        'scipy',
        'tkinter',
        'PyQt5',
        'PyQt6',
        'wx',
        'test',
        'unittest',
        'pydoc',
        'doctest',
        # 'email',  # Required by pydantic -> mcp
        # 'http.server',
        # 'ftplib',
        # 'telnetlib',
        # 'ssl',  # Required by anyio -> mcp
        'sqlite3',
    ],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

# Create the EXE without including binaries/datas (onedir mode)
exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name='mt5-quant',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=True,
    disable_windowed_traceback=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)

# Collect all files into directory (onedir mode)
coll = COLLECT(
    exe,
    a.binaries,
    a.zipfiles,
    a.datas,
    strip=False,
    upx=False,
    upx_exclude=[],
    name='mt5-quant'
)
