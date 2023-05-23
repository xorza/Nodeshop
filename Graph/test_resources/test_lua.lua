function sum(a, b)
    return a + b
end
sum_info = {
    name = "sum",
    inputs = {
        { "a", "f64" },
        { "b", "f64" }
    },
    outputs = {
        { "result", "f64" }
    },
    func = sum
}
register_function(sum_info)

function mult(a, b)
    return a * b
end
mult_info = {
    name = "mult",
    inputs = {
        { "a", "f64" },
        { "b", "f64" }
    },
    outputs = {
        { "result", "f64" }
    },
    func = mult
}
register_function(mult_info)

function val0()
    return 4
end
val0_info = {
    name = "val0",
    inputs = { },
    outputs = {
        { "result", "f64" }
    },
    func = val0
}
register_function(val0_info)

function val1()
    return 4
end
val1_info = {
    name = "val1",
    inputs = { },
    outputs = {
        { "result", "f64" }
    },
    func = val1
}
register_function(val1_info)

function print_func(t)
    print(t)
end
print_info = {
    name = "print",
    inputs = { { "a", "f64" } },
    outputs = { },
    func = print_func
}
register_function(print_info)

function graph()
    local a = val0()
    local b = val1()
    local c_sum_a_b = sum(a, b)
    local d_mult_b_c = mult(b, c_sum_a_b)
    print(d_mult_b_c)
end