long syscall(unsigned int n, unsigned long p1, unsigned long p2, unsigned long p3, unsigned long p4, unsigned long p5, unsigned long p6) {
    long ret;
    register unsigned long r10 asm("r10") = p4;
    register unsigned long r8 asm("r8") = p5;
    register unsigned long r9 asm("r9") = p6;
    asm volatile("syscall" : "=a"(ret) : "a"(n), "D"(p1), "S"(p2), "d"(p3), "r"(r10), "r"(r8), "r"(r9) : "rcx");
}

long write(int fd, const void *buf, unsigned long count) {
    return syscall(1, fd, (unsigned long) buf, count, 0, 0, 0);
}

long exit(int status) {
    return syscall(2, status, 0, 0, 0, 0, 0);
}

void* mmap(void* addr, unsigned long length, int prot, int flags, int file, long offset) {
    return (void*) syscall(3, (unsigned long) addr, length, prot, flags, file, offset);
}

long munmap(void* addr, unsigned long length) {
    return syscall(4, (unsigned long) addr, length, 0, 0, 0, 0);
}

void _start() {
    write(1, "Hello, world!\n", 14);
    const int SIZE = 0x100;
    int* buf = (int*) mmap((void*)0x32000, SIZE + 0x4000, 0x1, 0x1, 0, 0);
    syscall(0, 1, 2, 3, 4, 5, (unsigned long) buf);
    int count = SIZE / sizeof(int);
    for (int i = 0; i < count; i++) {
        buf[i] = i + 1;
    }
    for (int i = 0; i < count; i++) {
        if ((i % 0x10) == 0) {
            write(1, "\n", 1);
        }
        char ibuf[4];
        ibuf[0] = buf[i] / 100 % 10 + '0';
        ibuf[1] = buf[i] / 10 % 10 + '0';
        ibuf[2] = buf[i] % 10 + '0';
        ibuf[3] = '\t';
        write(1, ibuf, 4);
    }
    write(1, "\n", 1);
    munmap(buf + 3 * (0x1000 / sizeof(int)), SIZE);
    *buf = 0;
    exit(0);
}
