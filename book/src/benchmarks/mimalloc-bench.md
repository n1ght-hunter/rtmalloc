# mimalloc-bench Charts

Real-world program benchmarks from the main branch.

```uplot
{
  "title": "cfrac",
  "labels": ["cfrac"],
  "datasets": [
    { "label": "rt_nightly", "color": "#2ca02c", "data": [2.31] },
    { "label": "rt_std", "color": "#9467bd", "data": [2.55] },
    { "label": "mimalloc", "color": "#17becf", "data": [2.42] },
    { "label": "system", "color": "#888888", "data": [3.15] }
  ],
  "axes": { "x": "Benchmark", "y": "Time (s)" }
}
```

```uplot
{
  "title": "espresso",
  "labels": ["espresso"],
  "datasets": [
    { "label": "rt_nightly", "color": "#2ca02c", "data": [1.85] },
    { "label": "rt_std", "color": "#9467bd", "data": [2.01] },
    { "label": "mimalloc", "color": "#17becf", "data": [1.92] },
    { "label": "system", "color": "#888888", "data": [2.45] }
  ],
  "axes": { "x": "Benchmark", "y": "Time (s)" }
}
```
