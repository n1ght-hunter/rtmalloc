window.BENCHMARK_DATA = {
  "lastUpdate": 1770962751483,
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
          "id": "df2832efb1c7a6802518321eb2a5eebd8ece6974",
          "message": "try new benchmarks",
          "timestamp": "2026-02-13T16:33:53+13:00",
          "tree_id": "8058bdd3a9358dc601026e5dee41543f906773da",
          "url": "https://github.com/n1ght-hunter/rstcmalloc/commit/df2832efb1c7a6802518321eb2a5eebd8ece6974"
        },
        "date": 1770955617797,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "batch_1000/rstc_nightly/4096",
            "value": 14600.21,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nightly/512",
            "value": 11153.18,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nightly/64",
            "value": 11010.96,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nightly/8",
            "value": 11007.64,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nostd/4096",
            "value": 47828.98,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nostd/512",
            "value": 21227.18,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nostd/64",
            "value": 13738.93,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nostd/8",
            "value": 12390.75,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_percpu/4096",
            "value": 35988.13,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_percpu/512",
            "value": 33809.17,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_percpu/64",
            "value": 33200.36,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_percpu/8",
            "value": 32692.53,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_std/4096",
            "value": 13807.1,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_std/512",
            "value": 10428.33,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_std/64",
            "value": 10076.36,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_std/8",
            "value": 10078.2,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nightly/2048",
            "value": 23409.02,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nightly/256",
            "value": 22198.05,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nightly/32",
            "value": 22076.08,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nostd/2048",
            "value": 27942.5,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nostd/256",
            "value": 24749.94,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nostd/32",
            "value": 25090.05,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_percpu/2048",
            "value": 127284.06,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_percpu/256",
            "value": 32970.63,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_percpu/32",
            "value": 33174.17,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_std/2048",
            "value": 22067.48,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_std/256",
            "value": 20180.73,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_std/32",
            "value": 20207.15,
            "unit": "ns"
          },
          {
            "name": "cross_thread_free/rstc_nightly",
            "value": 486057.57,
            "unit": "ns"
          },
          {
            "name": "cross_thread_free/rstc_nostd",
            "value": 2235463.62,
            "unit": "ns"
          },
          {
            "name": "cross_thread_free/rstc_percpu",
            "value": 489147.3,
            "unit": "ns"
          },
          {
            "name": "cross_thread_free/rstc_std",
            "value": 469786.81,
            "unit": "ns"
          },
          {
            "name": "mixed_sizes/rstc_nightly",
            "value": 34054.03,
            "unit": "ns"
          },
          {
            "name": "mixed_sizes/rstc_nostd",
            "value": 41792.37,
            "unit": "ns"
          },
          {
            "name": "mixed_sizes/rstc_percpu",
            "value": 62254.79,
            "unit": "ns"
          },
          {
            "name": "mixed_sizes/rstc_std",
            "value": 32410.86,
            "unit": "ns"
          },
          {
            "name": "multithread_4t/rstc_nightly",
            "value": 247306.22,
            "unit": "ns"
          },
          {
            "name": "multithread_4t/rstc_nostd",
            "value": 4175269.7,
            "unit": "ns"
          },
          {
            "name": "multithread_4t/rstc_percpu",
            "value": 402952.67,
            "unit": "ns"
          },
          {
            "name": "multithread_4t/rstc_std",
            "value": 235195.88,
            "unit": "ns"
          },
          {
            "name": "producer_consumer/rstc_nightly",
            "value": 638039.12,
            "unit": "ns"
          },
          {
            "name": "producer_consumer/rstc_nostd",
            "value": 834016.56,
            "unit": "ns"
          },
          {
            "name": "producer_consumer/rstc_percpu",
            "value": 502081.82,
            "unit": "ns"
          },
          {
            "name": "producer_consumer/rstc_std",
            "value": 643875.81,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/1024",
            "value": 9.65,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/256",
            "value": 9.64,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/4096",
            "value": 11.51,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/64",
            "value": 9.64,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/65536",
            "value": 15.25,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/8",
            "value": 9.66,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/1024",
            "value": 11.22,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/256",
            "value": 11.21,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/4096",
            "value": 13.71,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/64",
            "value": 11.22,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/65536",
            "value": 17.44,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/8",
            "value": 11.23,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/1024",
            "value": 16.23,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/256",
            "value": 16.24,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/4096",
            "value": 13.69,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/64",
            "value": 16.22,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/65536",
            "value": 16.18,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/8",
            "value": 16.21,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/1024",
            "value": 9.02,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/256",
            "value": 9.02,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/4096",
            "value": 10.89,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/64",
            "value": 9.02,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/65536",
            "value": 14.62,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/8",
            "value": 9.02,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_nightly/1",
            "value": 93275.72,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_nightly/2",
            "value": 140147.75,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_nightly/4",
            "value": 208294.8,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_nightly/8",
            "value": 398201.19,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_nostd/1",
            "value": 96743.84,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_nostd/2",
            "value": 369998.51,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_nostd/4",
            "value": 2111026.95,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_nostd/8",
            "value": 7131277.07,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_percpu/1",
            "value": 151154.07,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_percpu/2",
            "value": 223930.29,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_percpu/4",
            "value": 279017.73,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_percpu/8",
            "value": 610461.07,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_std/1",
            "value": 91835.58,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_std/2",
            "value": 138223.37,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_std/4",
            "value": 206120.73,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_std/8",
            "value": 396666.88,
            "unit": "ns"
          },
          {
            "name": "vec_growth/rstc_nightly",
            "value": 5999.99,
            "unit": "ns"
          },
          {
            "name": "vec_growth/rstc_nostd",
            "value": 6082.57,
            "unit": "ns"
          },
          {
            "name": "vec_growth/rstc_percpu",
            "value": 6355.85,
            "unit": "ns"
          },
          {
            "name": "vec_growth/rstc_std",
            "value": 5992.77,
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
          "id": "059ef7688cc96803db3b93859de6dd3e2a481502",
          "message": "fix up allocator and workflows",
          "timestamp": "2026-02-13T18:32:45+13:00",
          "tree_id": "abdd00f2b7cc8993bf39b42b58ce1f2ccbe1a151",
          "url": "https://github.com/n1ght-hunter/rstcmalloc/commit/059ef7688cc96803db3b93859de6dd3e2a481502"
        },
        "date": 1770962751010,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "batch_1000/rstc_nightly/4096",
            "value": 14644.45,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nightly/512",
            "value": 11070.36,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nightly/64",
            "value": 10078.51,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nightly/8",
            "value": 10029.45,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nostd/4096",
            "value": 54241.02,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nostd/512",
            "value": 21867.12,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nostd/64",
            "value": 13827.58,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_nostd/8",
            "value": 12451.4,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_percpu/4096",
            "value": 34625.58,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_percpu/512",
            "value": 33088.83,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_percpu/64",
            "value": 33220.43,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_percpu/8",
            "value": 33067.91,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_std/4096",
            "value": 14450.56,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_std/512",
            "value": 11166.29,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_std/64",
            "value": 10053.91,
            "unit": "ns"
          },
          {
            "name": "batch_1000/rstc_std/8",
            "value": 9925.14,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nightly/2048",
            "value": 23805.93,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nightly/256",
            "value": 22849.27,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nightly/32",
            "value": 22806.9,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nostd/2048",
            "value": 29059.62,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nostd/256",
            "value": 26175.51,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_nostd/32",
            "value": 31196.3,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_percpu/2048",
            "value": 136397.55,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_percpu/256",
            "value": 36159.95,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_percpu/32",
            "value": 42413.39,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_std/2048",
            "value": 23910.55,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_std/256",
            "value": 22761.76,
            "unit": "ns"
          },
          {
            "name": "churn/rstc_std/32",
            "value": 22809.9,
            "unit": "ns"
          },
          {
            "name": "cross_thread_free/rstc_nightly",
            "value": 466240.27,
            "unit": "ns"
          },
          {
            "name": "cross_thread_free/rstc_nostd",
            "value": 2523544.27,
            "unit": "ns"
          },
          {
            "name": "cross_thread_free/rstc_percpu",
            "value": 482166.12,
            "unit": "ns"
          },
          {
            "name": "cross_thread_free/rstc_std",
            "value": 463052.89,
            "unit": "ns"
          },
          {
            "name": "mixed_sizes/rstc_nightly",
            "value": 36137.68,
            "unit": "ns"
          },
          {
            "name": "mixed_sizes/rstc_nostd",
            "value": 44697.21,
            "unit": "ns"
          },
          {
            "name": "mixed_sizes/rstc_percpu",
            "value": 63636.33,
            "unit": "ns"
          },
          {
            "name": "mixed_sizes/rstc_std",
            "value": 37363.95,
            "unit": "ns"
          },
          {
            "name": "multithread_4t/rstc_nightly",
            "value": 235082.89,
            "unit": "ns"
          },
          {
            "name": "multithread_4t/rstc_nostd",
            "value": 4767327.15,
            "unit": "ns"
          },
          {
            "name": "multithread_4t/rstc_percpu",
            "value": 388569.5,
            "unit": "ns"
          },
          {
            "name": "multithread_4t/rstc_std",
            "value": 235342.87,
            "unit": "ns"
          },
          {
            "name": "producer_consumer/rstc_nightly",
            "value": 610574.24,
            "unit": "ns"
          },
          {
            "name": "producer_consumer/rstc_nostd",
            "value": 808023.11,
            "unit": "ns"
          },
          {
            "name": "producer_consumer/rstc_percpu",
            "value": 483253.03,
            "unit": "ns"
          },
          {
            "name": "producer_consumer/rstc_std",
            "value": 634544.5,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/1024",
            "value": 9.03,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/256",
            "value": 9.04,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/4096",
            "value": 9.96,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/64",
            "value": 9.03,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/65536",
            "value": 11.82,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nightly/8",
            "value": 9.04,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/1024",
            "value": 11.32,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/256",
            "value": 11.32,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/4096",
            "value": 12.49,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/64",
            "value": 12.14,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/65536",
            "value": 14.33,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_nostd/8",
            "value": 11.79,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/1024",
            "value": 15.66,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/256",
            "value": 15.68,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/4096",
            "value": 14.16,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/64",
            "value": 15.71,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/65536",
            "value": 14.21,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_percpu/8",
            "value": 15.68,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/1024",
            "value": 9.02,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/256",
            "value": 9.03,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/4096",
            "value": 9.96,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/64",
            "value": 9.02,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/65536",
            "value": 11.83,
            "unit": "ns"
          },
          {
            "name": "single_alloc_dealloc/rstc_std/8",
            "value": 9.02,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_nightly/1",
            "value": 89678.74,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_nightly/2",
            "value": 132390.1,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_nightly/4",
            "value": 193093.99,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_nightly/8",
            "value": 399878.38,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_nostd/1",
            "value": 92900.45,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_nostd/2",
            "value": 402127.18,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_nostd/4",
            "value": 2443785.96,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_nostd/8",
            "value": 8468125.6,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_percpu/1",
            "value": 158831.24,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_percpu/2",
            "value": 206607.85,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_percpu/4",
            "value": 266611.82,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_percpu/8",
            "value": 553788.18,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_std/1",
            "value": 89578.45,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_std/2",
            "value": 133159.46,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_std/4",
            "value": 190809.33,
            "unit": "ns"
          },
          {
            "name": "thread_scalability/rstc_std/8",
            "value": 380935.17,
            "unit": "ns"
          },
          {
            "name": "vec_growth/rstc_nightly",
            "value": 6021.86,
            "unit": "ns"
          },
          {
            "name": "vec_growth/rstc_nostd",
            "value": 6083.01,
            "unit": "ns"
          },
          {
            "name": "vec_growth/rstc_percpu",
            "value": 6201.77,
            "unit": "ns"
          },
          {
            "name": "vec_growth/rstc_std",
            "value": 6031.53,
            "unit": "ns"
          }
        ]
      }
    ]
  }
}