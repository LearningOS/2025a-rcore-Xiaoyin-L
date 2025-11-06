
# CH5实验报告

## 一、本实验实现功能

1. 维护前面章节的实现系统调用`sys_get_time`，`sys_mmap`，`sys_unmap`，主要就是因为添加了进程的概念，需要修改之前的FLags变成MapPermission，以及创建新的map_area，由于后续还需要维护，所有利用分层思想，将映射解映射放到了`memory_set.rs`中。

2. 实现`spawn`功能，参考系统调用fork，exec以及initproc函数。

3. 实现`stride`调度算法。主要就是要在`manager.rs`中修改`fetch`函数，从而修改获取下一任务的逻辑。这里要注意在`current_task.inner_exclusive_access()`可能会存在的双重借用问题。

## 二、简答题

1. 不是，由于`p2`在执行后的`p2.stride = 250+10=260`，溢出，会变为`260%256 = 4 < p1.stride = 255`，所有还会是`p2`执行。
2. 证明：已知`priority>=2`，那么`pass = BigStride / priority <= BigStride/2`，则有两个进程的`stride`差值`stride_max – stride_min`，连续调度，也不会超过 `BigStride/2`.
3. 可以考虑利用前面提到的`BigStride/2`进行比较，代码如下:

   ```rust
   use core::cmp::Ordering;

    const BIG_STRIDE: u64 = 255;

    struct Stride(u64);

    impl PartialOrd for Stride {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            let a = self.0;
            let b = other.0;
            /* a.wrapping_sub(b)： a >= b 则 a - b； a < b 则 a - b + (最大值 + 1) */
            let diff = (a.wrapping_sub(b)) % BIG_STRIDE;
            if diff < BIG_STRIDE / 2 {
                Some(Ordering::Less)
            } else {
                Some(Ordering::Greater)
            }
        }
    }

    impl PartialEq for Stride {
        fn eq(&self, _other: &Self) -> bool {
            false
        }
    }
    ```

## 三、荣誉准则

1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：
   参考了原有的系统调用`fork,exec,initproc`的实现。

2. 此外，我也参考了以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：
   无。

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。

## 四、Optional
