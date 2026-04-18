# -*- mode: python ; coding: utf-8 -*-
"""
PyInstaller spec file for MT5-Quant MCP Server (onedir mode)
This version creates a directory with the executable + dependencies
Better for MCP stdio communication than onefile mode

Build:
  pyinstaller mt5-quant-onedir.spec

Output:
  dist/mt5-quant/ directory with executable inside
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
hiddenimports = mcp_hiddenimports + [
    # MCP stdio - explicitly include for PyInstaller
    'mcp.server.stdio',
    'mcp.shared.memory',
    'mcp.shared.session',
    'mcp.server.models',
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
    # Other stdlib that might be dynamically imported
    'xml.etree.ElementTree',
    'html.parser',
]

a = Analysis(
    ['server/main.py'],
    pathex=[str(root), str(root / 'server'), str(root / 'analytics')],
    binaries=mcp_binaries,
    datas=datas,
    hiddenimports=hiddenimports,
    hookspath=[],
    hooksconfig={},
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
        'email',
        'http.server',
        'ftplib',
        'telnetlib',
        'ssl',
        'sqlite3',
    ],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name='mt5-quant',
    debug=False,
    bootloader_ignore_signals=False,
    strip=True,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=True,
    disable_windowed_traceback=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)

# For onedir mode, we need to use COLLECT
# But actually, on macOS/Linux, just having the EXE without onefile=True creates a directory
# Wait, actually we need to check PyInstaller version behavior
# Modern PyInstaller creates onedir by default for EXE + COLLECT
