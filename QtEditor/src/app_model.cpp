
#include "app_model.hpp"

#include "graph_c_api.hpp"

#include <QtQml>

AppModel::AppModel() {
    qmlRegisterUncreatableType<QmlFunctionInfo>("com.csso", 1, 0, "QmlFunctionInfo", "");
    qmlRegisterUncreatableType<QmlArgInfo>("com.csso", 1, 0, "QmlArgInfo", "");
    
    graph_c_api::init();

    auto functions = graph_c_api::get_functions();
    for (auto &func: functions) {
        m_functions.append(new QmlFunctionInfo(func, this));
    }
}

AppModel::~AppModel() {
    graph_c_api::deinit();
}