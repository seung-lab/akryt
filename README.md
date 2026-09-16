# akryt

Realtime high-throughput image tile compression.

Electron microscopy datasets are getting too big. Offline compression doesn't solve the problem. We need reliable realtime transcoding.

## Project Mission

Develop the capability to perform realtime transcoding of uncompressed image tiles to JEPG-XL for multiple electron microscopy imaging processes at 400 MB/sec per a microscope over four microscopes (1.6 GB/sec or 12.8 Gbps). The results should be written to object storage.

The overal goal is to reduce usage by a factor of 5x to 20x on the storage device with lossy compression but at least 20 to 50% with lossless compression.

## Subgoals

1. The realtime process should be robust to external network or destination storage failure and be able to switch target to local hard disk when needed. It should be able to evacuate data from hard disk simultaneously with accepting imaging at 100% of the expected volume. 
2. The type of processing should be configurable per a flow (i.e. microscope 1 might have different requirements from microscope 2).
3. The machine status should be monitorable.
4. If akryt goes down, it should be possible to bypass it and write directly to the storage device.

## About the Name

"akryt" is a phonetic misspelling of "accrete" as in the accretion disk of a gravitational singularity. In our compression pipeline, a large fast flow of images are condensed and the process develops heat as a byproduct. The resultant images then accumulate at the destination. 
