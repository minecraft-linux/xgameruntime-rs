#ifdef _WIN64
typedef unsigned long ULONG;

int _tls_index = 0;

__attribute__((section(".tls"))) char _tls_start = 0;
__attribute__((section(".tls$ZZZ"))) char _tls_end = 0;

__attribute__((section(".CRT$XLA"))) void *__xl_a = 0;
__attribute__((section(".CRT$XLZ"))) void *__xl_z = 0;

const struct {
    void *StartAddressOfRawData;
    void *EndAddressOfRawData;
    void *AddressOfIndex;
    void *AddressOfCallBacks;
    ULONG SizeOfZeroFill;
    ULONG Characteristics;
} _tls_used = {
    &_tls_start,
    &_tls_end,
    &_tls_index,
    &__xl_a + 1,
};
#endif
