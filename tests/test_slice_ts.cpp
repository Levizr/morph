#include "../../morph/runtime/types/js_types.h"
#include <iostream>
#include <print>

__attribute__((constructor)) static void _morph_io_init() {
    std::ios_base::sync_with_stdio(false);
    std::cin.tie(NULL);
}

int main()
{
    JsNumber num = 1234;
    std::println("{}", num.toString().charAt(0).toUpperCase() + num.toString().slice(1));
    return 0;
}
