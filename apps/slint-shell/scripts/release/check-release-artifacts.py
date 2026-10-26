#!/usr/bin/env python3
"""Read-only verification of assembled Linux/Windows distribution artifacts."""
import argparse
import hashlib
from pathlib import Path, PurePosixPath
import shutil
import struct
import subprocess
import tarfile
import tomllib
import zipfile


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def verify_manifest(directory, name):
    entries = set()
    for line in (directory / name).read_text().splitlines():
        checksum, filename = line.split('  ', 1)
        path = PurePosixPath(filename)
        if len(path.parts) != 1 or path.is_absolute() or filename in entries:
            raise ValueError(f'Unsafe or duplicate checksum entry in {name}')
        if len(checksum) != 64 or any(c not in '0123456789abcdefABCDEF' for c in checksum):
            raise ValueError(f'Invalid checksum in {name}')
        if digest(directory / filename) != checksum.lower():
            raise ValueError(f'Checksum mismatch: {filename}')
        entries.add(filename)
    return entries


def verify_binary(data, windows=False):
    if windows:
        if data[:2] != b'MZ' or len(data) < 64:
            raise ValueError('Not a PE executable')
        offset = struct.unpack_from('<I', data, 60)[0]
        if data[offset:offset + 4] != b'PE\0\0' or struct.unpack_from('<H', data, offset + 4)[0] != 0x8664:
            raise ValueError('Windows executable is not x64 PE')
    elif data[:6] != b'\x7fELF\x02\x01' or struct.unpack_from('<H', data, 18)[0] != 62:
        raise ValueError('Linux executable is not little-endian x86_64 ELF')


def check(directory, version, require_vendor=False, compare_linux=False, lockfile=None):
    linux = verify_manifest(directory, 'SHA256SUMS')
    windows = verify_manifest(directory, 'WINDOWS-SHA256SUMS')
    archive = f'left-hand-control-{version}-linux-x86_64.tar.gz'
    deb = f'left-hand-control_{version}_amd64.deb'
    rpm = f'left-hand-control-{version}-1.x86_64.rpm'
    portable = f'left-hand-control-{version}-windows-x86_64.zip'
    if not {archive, deb, rpm} <= linux or windows != {portable}:
        raise ValueError('Distribution files/version missing from checksum manifests')
    if require_vendor and f'left-hand-control-{version}-vendor.tar.xz' not in linux:
        raise ValueError('Offline vendor source archive is missing')
    if require_vendor:
        root = f'left-hand-control-{version}-vendor'
        with tarfile.open(directory / f'{root}.tar.xz') as tf:
            names = {m.name for m in tf.getmembers() if m.isfile()}
            config = tomllib.loads(tf.extractfile(f'{root}/.cargo/config.toml').read().decode())
            if config.get('source', {}).get('vendored-sources', {}).get('directory') != 'vendor':
                raise ValueError('Vendor config does not use its relative vendor directory')
            lock = tomllib.loads((lockfile or Path(__file__).resolve().parents[4] / 'Cargo.lock').read_text())
            for package in lock['package']:
                if not package.get('source', '').startswith('registry+'):
                    continue
                crate = f"{root}/vendor/{package['name']}-{package['version']}"
                if not {f'{crate}/Cargo.toml', f'{crate}/.cargo-checksum.json'} <= names:
                    raise ValueError(f"Vendored dependency missing: {package['name']} {package['version']}")
    with tarfile.open(directory / archive) as tf:
        files = [m for m in tf.getmembers() if m.isfile()]
        executables = [m for m in files if PurePosixPath(m.name).name == 'left-hand-control']
        if len(executables) != 1:
            raise ValueError('Linux archive must contain one executable')
        binary = tf.extractfile(executables[0]).read()
        verify_binary(binary)
        names = {PurePosixPath(m.name).name for m in files}
        if not {'LICENSE', 'install-linux.sh'} <= names:
            raise ValueError('Linux archive lacks license/installer')
    with zipfile.ZipFile(directory / portable) as zf:
        if zf.testzip():
            raise ValueError('ZIP CRC mismatch')
        files = [n for n in zf.namelist() if not n.endswith('/')]
        entries = {n.replace('\\', '/'): n for n in files}
        if len(entries) != len(files):
            raise ValueError('Duplicate normalized ZIP paths')
        exe = [n for n in entries if PurePosixPath(n).name == 'left-hand-control.exe']
        if len(exe) != 1:
            raise ValueError('ZIP must contain one executable')
        verify_binary(zf.read(entries[exe[0]]), windows=True)
        parent = str(PurePosixPath(exe[0]).parent)
        for name in ('LICENSE', 'README.md', 'features.md'):
            if str(PurePosixPath(parent) / name) not in entries:
                raise ValueError(f'ZIP lacks {name} beside executable')
    if (directory / deb).read_bytes()[:8] != b'!<arch>\n':
        raise ValueError('Invalid DEB archive magic')
    if (directory / rpm).read_bytes()[:4] != b'\xed\xab\xee\xdb':
        raise ValueError('Invalid RPM magic')
    if compare_linux:
        if not shutil.which('bsdtar'):
            raise ValueError('--compare-linux requires the existing bsdtar tool')
        for package in (deb, rpm):
            listing = subprocess.check_output(['bsdtar', '-tf', str(directory / package)], text=True)
            if package == deb:
                member = next(n for n in listing.splitlines() if n.startswith('data.tar'))
                data = subprocess.check_output(['bsdtar', '-xOf', str(directory / package), member])
                listing = subprocess.check_output(['bsdtar', '-tf', '-'], input=data).decode()
                member = next(n for n in listing.splitlines() if n.lstrip('./').rstrip('/') == 'usr/bin/left-hand-control')
                extracted = subprocess.check_output(['bsdtar', '-xOf', '-', member], input=data)
            else:
                member = next(n for n in listing.splitlines() if n.lstrip('./').rstrip('/') == 'usr/bin/left-hand-control')
                extracted = subprocess.check_output(['bsdtar', '-xOf', str(directory / package), member])
            if extracted != binary:
                raise ValueError(f'{package} binary differs from the accepted tar executable')
    return f'{version}: checksums, required files, ELF/PE architecture, package formats; Linux binary comparison={compare_linux}'


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--require-vendor', action='store_true')
    parser.add_argument('--compare-linux', action='store_true')
    args = parser.parse_args()
    try:
        print('RELEASE ARTIFACTS PASSED:', check(args.directory, args.version, args.require_vendor, args.compare_linux))
    except (ValueError, OSError, StopIteration, subprocess.SubprocessError, zipfile.BadZipFile, tarfile.TarError) as error:
        parser.exit(1, f'RELEASE ARTIFACTS FAILED: {error}\n')
