# FastCDC in SDAL

SDAL uses **FastCDC (Fast Content-Defined Chunking)** to split files into chunks. Unlike fixed-size chunking (where files are split every *N* bytes), content-defined chunking splits files based on limits defined by the content itself.

## Why FastCDC?

1.  **Deduplication**: If you insert a byte at the beginning of a file, fixed-size chunking would shift every subsequent chunk, changing all their hashes. FastCDC aligns chunks based on content patterns, so only the changed chunk is affected. The rest of the file remains identical in terms of chunks, allowing SDAL to reuse existing blobs.
2.  **Efficiency**: SDAL stores chunks in a Content-Addressable Store (CAS). By maximizing reuse, we store significantly less data for file modifications.

## Configuration

SDAL uses a deterministic configuration for FastCDC to ensure that the same file always produces the same chunks across all machines.

-   **Minimum Chunk Size**: 16 KB (16,384 bytes)
-   **Average Chunk Size**: 64 KB (65,536 bytes)
-   **Maximum Chunk Size**: 1 MB (1,048,576 bytes)

## How it Works

1.  **Scanning**: The algorithm scans the byte stream of a file.
2.  **Cut Points**: It calculates a rolling hash. When the hash satisfies a specific condition (determined by the average size parameter), a "cut point" is made, creating a chunk.
3.  **Constraints**: It forces a cut if the chunk reaches the *Maximum Size* and prevents a cut if the chunk is smaller than the *Minimum Size*.
