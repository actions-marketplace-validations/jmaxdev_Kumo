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

