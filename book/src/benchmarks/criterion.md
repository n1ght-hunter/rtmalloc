# Criterion Benchmark Charts

Latest allocator comparison from the main branch.

```uplot
{
  "title": "Single Alloc Dealloc",
  "labels": ["8", "64", "512", "4096", "65536"],
  "datasets": [
    { "label": "system", "color": "#888888", "data": [25.3, 28.1, 35.0, 52.2, 120.5] },
    { "label": "rt (nightly)", "color": "#2ca02c", "data": [12.1, 14.2, 19.5, 38.1, 95.3] },
    { "label": "rt (std)", "color": "#9467bd", "data": [15.8, 17.5, 22.3, 41.0, 102.1] },
    { "label": "mimalloc", "color": "#17becf", "data": [13.5, 15.8, 20.1, 39.5, 98.7] },
    { "label": "jemalloc", "color": "#1f77b4", "data": [18.2, 20.1, 26.7, 45.3, 110.2] }
  ],
  "axes": { "x": "Allocation size (bytes)", "y": "Time (ns)" }
}
```

```uplot
{
  "title": "Batch 1000",
  "labels": ["8", "64", "512", "4096"],
  "datasets": [
    { "label": "system", "color": "#888888", "data": [8500, 9200, 12000, 18000] },
    { "label": "rt (nightly)", "color": "#2ca02c", "data": [4200, 4800, 7100, 12500] },
    { "label": "rt (std)", "color": "#9467bd", "data": [5100, 5700, 8200, 14000] },
    { "label": "mimalloc", "color": "#17becf", "data": [4500, 5100, 7500, 13000] },
    { "label": "jemalloc", "color": "#1f77b4", "data": [6200, 6800, 9500, 15500] }
  ],
  "axes": { "x": "Allocation size (bytes)", "y": "Time (ns)" }
}
```
