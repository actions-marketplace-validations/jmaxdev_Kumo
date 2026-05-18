## Benchmarks results

Important: these benchmarks were run on a local machine and can vary depending on the machine's and packages installed or downloaded. So, use this as a reference.

### Cold run (no caches)

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `npm install` | 57.424 ± 3.572 | 54.898 | 59.950 | 1.99 ± 0.12 |
| `pnpm install` | 91.294 ± 4.838 | 87.873 | 94.715 | 3.16 ± 0.17 |
| `bun install` | 28.869 ± 0.220 | 28.713 | 29.025 | 1.00 |
| `kumo install` | 38.384 ± 0.389 | 38.109 | 38.659 | 1.33 ± 0.02 |

### Warm run (with caches)

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `npm install` | 39.942 ± 2.762 | 37.989 | 41.895 | 2.15 ± 0.15 |
| `pnpm install` | 43.464 ± 6.528 | 38.848 | 48.080 | 2.34 ± 0.35 |
| `bun install` | 18.537 ± 0.258 | 18.354 | 18.720 | 1.00 |
| `kumo install` | 21.497 ± 0.862 | 20.888 | 22.107 | 1.16 ± 0.05 |

### Build benchmark results

| Tool          | Version | Time (mean ± σ)           | Comparison | JS      | CSS       | Sourcemaps |
| ------------- | ------- | ------------------------: | ---------- | ------- | --------- | ---------- |
| kumo:rollup   | 0.2.1   |        951.18 ±  23.77 ms | 1.0x       | 5.33 MB | not found | 12.92 MB   |
| kumo:rolldown | 0.2.1   |        951.62 ±  22.10 ms | 1.0x       | 5.22 MB | not found | 13.38 MB   |
| kumo:rspack   | 0.2.1   |        959.00 ±  26.23 ms | 1.0x       | 5.17 MB | not found | 12.76 MB   |
| kumo:bun      | 0.2.1   |        959.41 ±  25.17 ms | 1.0x       | 5.34 MB | not found | 13.11 MB   |
| kumo:esbuild  | 0.2.1   |        961.31 ±  24.22 ms | 1.0x       | 5.90 MB | 38 B      | 14.77 MB   |
| kumo:vite     | 0.2.1   |        967.41 ±  34.50 ms | 1.0x       | 5.20 MB | 1 B       | 13.21 MB   |
| kumo:rsbuild  | 0.2.1   |        985.12 ±  46.07 ms | 1.0x       | 5.17 MB | not found | 12.59 MB   |
| rolldown      | 1.0.1   |       1603.30 ±  22.60 ms | 1.7x       | 5.22 MB | not found | 13.38 MB   |
| vite          | 8.0.13  |       2061.97 ±  55.36 ms | 2.2x       | 5.20 MB | 1 B       | 13.21 MB   |
| bun           | 1.3.14  |       2270.79 ±  71.53 ms | 2.4x       | 5.34 MB | not found | 13.11 MB   |
| esbuild       | 0.28.0  |       2929.91 ±  24.66 ms | 3.1x       | 5.90 MB | 38 B      | 14.77 MB   |
| rspack        | 2.0.3   |       3361.95 ± 110.02 ms | 3.5x       | 5.17 MB | not found | 12.76 MB   |
| rsbuild       | 2.0.6   |       3637.28 ± 120.24 ms | 3.8x       | 5.17 MB | not found | 12.59 MB   |
| rollup        | 4.60.4  |     63788.59 ± 1261.40 ms | 67.1x      | 5.33 MB | not found | 12.92 MB   |
