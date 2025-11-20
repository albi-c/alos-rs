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

void _start() {
    syscall(0, 1, 2, 3, 4, 5, 6);
    write(1, "Hello, world!\n", 14);
    exit(0);
}
