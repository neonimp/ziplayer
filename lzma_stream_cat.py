#!/usr/bin/env python3
# encoding=utf-8

import lzma
import sys


def main():
    if not len(sys.argv) == 3:
        print('Usage: lzma_stream_cat.py <file> <out>')
        sys.exit(1)
    # Open the file
    with lzma.open(sys.argv[1], 'r') as f:
        out = f.read()

    with open(sys.argv[2], 'wb') as f:
        f.write(out)

    print('Done.')


if __name__ == '__main__':
    print('LZMA Stream Cat')
    print('Copies a LZMA stream to a file while decompressing it.')
    print('Written by Matheus Xavier (c) 2023')
    main()
