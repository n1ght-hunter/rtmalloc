window.BENCHMARK_DATA = {
  "lastUpdate": 1770910272856,
  "repoUrl": "https://github.com/n1ght-hunter/rstcmalloc",
  "entries": {
    "rstcmalloc Benchmarks": [
      {
        "commit": {
          "author": {
            "email": "samuelhuntnz@gmail.com",
            "name": "Samuel Hunt",
            "username": "n1ght-hunter"
          },
          "committer": {
            "email": "samuelhuntnz@gmail.com",
            "name": "Samuel Hunt",
            "username": "n1ght-hunter"
          },
          "distinct": true,
          "id": "49bd19e76d48dff5ae998ee4b197f36a9860f0ef",
          "message": "update mallocs benchmark",
          "timestamp": "2026-02-13T03:58:47+13:00",
          "tree_id": "2aaba4a200b2a99c9414fbb9e83ea483bc94365e",
          "url": "https://github.com/n1ght-hunter/rstcmalloc/commit/49bd19e76d48dff5ae998ee4b197f36a9860f0ef"
        },
        "date": 1770909825610,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "batch_1000/rstc_nightly/4096",
            "value": 14288.36,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nightly/512",
            "value": 10998.86,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nightly/64",
            "value": 10366.01,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nightly/8",
            "value": 10363.79,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nostd/4096",
            "value": 46911.79,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nostd/512",
            "value": 21334.15,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nostd/64",
            "value": 13606.98,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nostd/8",
            "value": 12477.17,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_percpu/4096",
            "value": 34271.9,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_percpu/512",
            "value": 33311.54,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_percpu/64",
            "value": 33398.1,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_percpu/8",
            "value": 33625.68,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_std/4096",
            "value": 13464.28,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_std/512",
            "value": 9955.64,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_std/64",
            "value": 9763.07,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_std/8",
            "value": 9856.08,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nightly/2048",
            "value": 23318.15,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nightly/256",
            "value": 22052.21,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nightly/32",
            "value": 22060.19,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nostd/2048",
            "value": 28132.02,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nostd/256",
            "value": 25022.55,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nostd/32",
            "value": 25208.02,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_percpu/2048",
            "value": 127495.35,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_percpu/256",
            "value": 130501.33,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_percpu/32",
            "value": 129772.39,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_std/2048",
            "value": 21441.67,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_std/256",
            "value": 20230.37,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_std/32",
            "value": 20186.44,
            "unit": "ns"
          },
          {
            "name": "multithread_4t/rstc_nightly",
            "value": 241246.06,
            "unit": "ns"
          },
          {
            "name": "multithread_4t/rstc_nostd",
            "value": 4256430.79,
            "unit": "ns"
          },
          {
            "name": "multithread_4t/rstc_percpu",
            "value": 391828.09,
            "unit": "ns"
          },
          {
            "name": "multithread_4t/rstc_std",
            "value": 233022.07,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/1024",
            "value": 9.03,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/256",
            "value": 9.03,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/4096",
            "value": 10.89,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/64",
            "value": 9.06,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/65536",
            "value": 14.63,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/8",
            "value": 9.03,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/1024",
            "value": 10.66,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/256",
            "value": 10.67,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/4096",
            "value": 13.08,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/64",
            "value": 10.67,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/65536",
            "value": 16.82,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/8",
            "value": 10.66,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/1024",
            "value": 16.25,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/256",
            "value": 16.26,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/4096",
            "value": 14.01,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/64",
            "value": 16.13,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/65536",
            "value": 15.56,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/8",
            "value": 16.24,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/1024",
            "value": 8.09,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/256",
            "value": 8.1,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/4096",
            "value": 9.96,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/64",
            "value": 8.09,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/65536",
            "value": 13.69,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/8",
            "value": 8.09,
            "unit": "ns"
          },
          {
            "name": "vec_growth/rstc_nightly",
            "value": 6012.65,
            "unit": "ns"
          },
          {
            "name": "vec_growth/rstc_nostd",
            "value": 6150.19,
            "unit": "ns"
          },
          {
            "name": "vec_growth/rstc_percpu",
            "value": 6188.33,
            "unit": "ns"
          },
          {
            "name": "vec_growth/rstc_std",
            "value": 6016.43,
            "unit": "ns"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "samuelhuntnz@gmail.com",
            "name": "Samuel Hunt",
            "username": "n1ght-hunter"
          },
          "committer": {
            "email": "samuelhuntnz@gmail.com",
            "name": "Samuel Hunt",
            "username": "n1ght-hunter"
          },
          "distinct": true,
          "id": "49d0da608b865a25a5c5b00c8c070c34a78ee665",
          "message": "fmt",
          "timestamp": "2026-02-13T04:07:48+13:00",
          "tree_id": "757a2f9dd2c1cd40de1f967e6e99e7cafcee5991",
          "url": "https://github.com/n1ght-hunter/rstcmalloc/commit/49d0da608b865a25a5c5b00c8c070c34a78ee665"
        },
        "date": 1770910272288,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "batch_1000/rstc_nightly/4096",
            "value": 14247.41,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nightly/512",
            "value": 11005.37,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nightly/64",
            "value": 10352.41,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nightly/8",
            "value": 10366.64,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nostd/4096",
            "value": 47139.87,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nostd/512",
            "value": 20661.86,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nostd/64",
            "value": 13371.78,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nostd/8",
            "value": 12232.91,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_percpu/4096",
            "value": 34587.77,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_percpu/512",
            "value": 33338.8,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_percpu/64",
            "value": 33631.75,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_percpu/8",
            "value": 32777.11,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_std/4096",
            "value": 13385.5,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_std/512",
            "value": 10123.46,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_std/64",
            "value": 9545.82,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_std/8",
            "value": 9474.8,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nightly/2048",
            "value": 23291.6,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nightly/256",
            "value": 22038.43,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nightly/32",
            "value": 22066.72,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nostd/2048",
            "value": 28990.52,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nostd/256",
            "value": 25054.12,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nostd/32",
            "value": 25445.75,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_percpu/2048",
            "value": 31701.33,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_percpu/256",
            "value": 32653.51,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_percpu/32",
            "value": 32482.96,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_std/2048",
            "value": 21449.21,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_std/256",
            "value": 20177.88,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_std/32",
            "value": 20192.42,
            "unit": "ns"
          },
          {
            "name": "multithread_4t/rstc_nightly",
            "value": 290751.67,
            "unit": "ns"
          },
          {
            "name": "multithread_4t/rstc_nostd",
            "value": 4489153.55,
            "unit": "ns"
          },
          {
            "name": "multithread_4t/rstc_percpu",
            "value": 420086.29,
            "unit": "ns"
          },
          {
            "name": "multithread_4t/rstc_std",
            "value": 226520.87,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/1024",
            "value": 9.02,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/256",
            "value": 9.02,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/4096",
            "value": 10.88,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/64",
            "value": 9.32,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/65536",
            "value": 14.61,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/8",
            "value": 9.02,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/1024",
            "value": 10.65,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/256",
            "value": 10.66,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/4096",
            "value": 13.07,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/64",
            "value": 10.66,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/65536",
            "value": 16.8,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/8",
            "value": 10.7,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/1024",
            "value": 16.23,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/256",
            "value": 16.25,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/4096",
            "value": 13.06,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/64",
            "value": 16.21,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/65536",
            "value": 15.54,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/8",
            "value": 16.22,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/1024",
            "value": 8.08,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/256",
            "value": 8.08,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/4096",
            "value": 9.95,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/64",
            "value": 8.09,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/65536",
            "value": 13.68,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/8",
            "value": 8.08,
            "unit": "ns"
          },
          {
            "name": "vec_growth/rstc_nightly",
            "value": 6011.71,
            "unit": "ns"
          },
          {
            "name": "vec_growth/rstc_nostd",
            "value": 6119.05,
            "unit": "ns"
          },
          {
            "name": "vec_growth/rstc_percpu",
            "value": 6187.12,
            "unit": "ns"
          },
          {
            "name": "vec_growth/rstc_std",
            "value": 6006.01,
            "unit": "ns"
          }
        ]
      }
    ]
  }
}