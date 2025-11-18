#!/bin/sh

nasm -f elf64 test_program.asm -o test_program.o && ld -static test_program.o -o test_program.elf
