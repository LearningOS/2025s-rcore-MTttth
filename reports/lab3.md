## 功能总结
### 迁移上一章的 ```sys_get_time``` ```sys_mmap``` ```sys_munmap``` 以适应新的进程结构。
本章的进程结构变化并不大，核心结构并没有改变，迁移较为容易。

### 实现了 ```sys_spawn``` 系统调用
该系统调用通过加载目标程序的 ```elf_data``` 完成地址空间的分配与子进程的创建，并执行目标程序。
对父程序成功返回子进程 ```id```，否则返回 ```-1```。对子进程返回 ```0```。

### 实现 stride 调度算法
- 首先为 ```TCB``` 加入了 ```prio```, ```pass```, ```stride```字段。
- 完成系统调用 ```sys_set_priority```
- 修改 ```run_tasks```的逻辑，每次需要调度时，从当前 ```runnable``` 态的进程中选择 ```stride``` 最小的进程调度。对于获得调度的进程 ```P```，将对应的 ```stride``` 加上其对应的步长 ```pass```。

## 问答题
### 实际情况是轮到 p1 执行吗？为什么？
并不是，```p2```执行完成后 ```stride += pass```导致 ```stride```溢出，真实数值为 ```4```。
故下一次调度还是 ```p2```。

### 简单说明
- 初始状态满足 ```STRIDE = 0```
$$
STRIDE_{max} – STRIDE_{min} <= BigStride / 2
$$
- 假设第 ```k```次调度满足
$$
STRIDE_{max}^t – STRIDE_{min}^t <= BigStride / 2
$$
- 在第 ```k + 1```次调度时,有
$$
STRIDE_{max}^{t+1} = max\{STRIDE_{max}^t, STRIDE_{min}^t + BigStride /prio\} \le max\{STRIDE_{max}^t, STRIDE_{min}^t + BigStride /2\}
$$
则有
$$
STRIDE_{max}^{t+1} - STRIDE_{min}^{t+1} \le BigStride / 2
$$

### partial_cmp 函数
```rust
impl PartialOrd for Stride {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // wrapping_sub will correctly handle overflow
        let diff = self.0.wrapping_sub(other.0);
        if diff == 0 {
            None
        } else if diff < (1 << 63) {
            Some(Ordering::Greater)
        } else {
            Some(Ordering::Less)
        }
    }
}
```

## 荣誉准则

1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

         使用了 Chatgpt 交流了有关汇编与Rust语法上的问题
         借助了 AI 助教理解实验框架

2. 此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

         《rCore-Tutorial-Guide-2025S》
			《rCore-Tutorial-Book-v3》

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。